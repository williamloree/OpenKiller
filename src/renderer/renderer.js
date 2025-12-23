/**
 * Open Killer - Renderer Process
 * Logique frontend pour l'interface utilisateur
 */

// === État de l'application ===
let allPorts = [];
let filteredPorts = [];
let sortColumn = "port";
let sortDirection = "asc";
let currentKillPid = null;
let currentKillPort = null;
let currentKillProcess = null;
let currentPage = "ports"; // "ports" ou "favorites"

// Ports favoris - chargés depuis localStorage
let favoritePorts = [];

// === Initialisation ===
document.addEventListener("DOMContentLoaded", () => {
  console.log("Application initialisée");
  initializeApp();
});

/**
 * Initialise l'application et tous les écouteurs d'événements
 */
function initializeApp() {
  // Charger les favoris depuis localStorage
  loadFavorites();

  // Charger les ports au démarrage
  loadPorts();

  // Configurer les écouteurs d'événements
  setupEventListeners();

  // Afficher les ports favoris
  renderFavorites();

  // Mettre à jour les badges
  updateTabBadges();

  // Actualiser automatiquement toutes les 10 secondes
  setInterval(loadPorts, 10000);
}

/**
 * Configure tous les écouteurs d'événements de l'interface
 */
function setupEventListeners() {
  // Navigation par onglets
  const tabs = document.querySelectorAll(".tab");
  tabs.forEach((tab) => {
    tab.addEventListener("click", () => {
      const tabName = tab.dataset.tab;
      switchTab(tabName);
    });
  });

  // Bouton d'actualisation
  const refreshBtn = document.getElementById("refreshBtn");
  refreshBtn.addEventListener("click", () => {
    loadPorts();
    showToast("Actualisation en cours...", "info");
  });

  // Champ de recherche
  const searchInput = document.getElementById("searchInput");
  searchInput.addEventListener("input", handleSearch);

  // Bouton d'effacement de recherche
  const clearSearch = document.getElementById("clearSearch");
  clearSearch.addEventListener("click", () => {
    searchInput.value = "";
    handleSearch();
    searchInput.focus();
  });

  // En-têtes triables
  const sortableHeaders = document.querySelectorAll(".sortable");
  sortableHeaders.forEach((header) => {
    header.addEventListener("click", () => {
      const column = header.dataset.sort;
      handleSort(column);
    });
  });

  // Bouton de confirmation du modal
  const confirmKillBtn = document.getElementById("confirmKillBtn");
  confirmKillBtn.addEventListener("click", confirmKillProcess);

  // Bouton d'ajout de favori
  const addFavoriteBtn = document.getElementById("addFavoriteBtn");
  addFavoriteBtn.addEventListener("click", showAddFavoriteModal);

  // Bouton de sauvegarde du favori
  const saveFavoriteBtn = document.getElementById("saveFavoriteBtn");
  saveFavoriteBtn.addEventListener("click", saveFavorite);

  // Fermeture du modal avec la touche Échap
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      closeModal();
      closeAddFavoriteModal();
    }
  });

  // Clic en dehors du modal pour fermer
  const modal = document.getElementById("confirmModal");
  modal.addEventListener("click", (e) => {
    if (e.target === modal) {
      closeModal();
    }
  });

  const addFavoriteModal = document.getElementById("addFavoriteModal");
  addFavoriteModal.addEventListener("click", (e) => {
    if (e.target === addFavoriteModal) {
      closeAddFavoriteModal();
    }
  });
}

/**
 * Charge la liste des ports depuis le processus principal
 */
async function loadPorts() {
  const loadingState = document.getElementById("loadingState");
  const emptyState = document.getElementById("emptyState");
  const tableBody = document.getElementById("portsTableBody");

  try {
    // Afficher l'indicateur de chargement
    loadingState.style.display = "flex";
    emptyState.style.display = "none";

    // Récupérer les ports via l'API Electron
    allPorts = await window.electronAPI.getPorts();

    // Appliquer le filtre de recherche actuel
    const searchTerm = document.getElementById("searchInput").value;
    filterPorts(searchTerm);

    // Trier les ports
    sortPorts();

    // Afficher les ports dans le tableau
    renderTable();

    // Mettre à jour les favoris
    updateFavorites();

    // Mettre à jour les badges des onglets
    updateTabBadges();

    // Cacher l'indicateur de chargement
    loadingState.style.display = "none";

    // Afficher l'état vide si nécessaire
    if (filteredPorts.length === 0) {
      emptyState.style.display = "flex";
    }
  } catch (error) {
    console.error("Erreur lors du chargement des ports:", error);
    loadingState.style.display = "none";
    showToast("Erreur lors du chargement des ports", "error");
  }
}

/**
 * Filtre les ports en fonction du terme de recherche
 * @param {string} searchTerm - Terme de recherche
 */
function filterPorts(searchTerm) {
  const term = searchTerm.toLowerCase().trim();

  if (!term) {
    filteredPorts = [...allPorts];
    updateFilterStatus("");
    return;
  }

  filteredPorts = allPorts.filter((port) => {
    const portStr = String(port.port).toLowerCase();
    const processStr = (port.processName || "").toLowerCase();
    const addressStr = (port.address || "").toLowerCase();

    return (
      portStr.includes(term) ||
      processStr.includes(term) ||
      addressStr.includes(term)
    );
  });

  updateFilterStatus(
    `${filteredPorts.length} résultat${
      filteredPorts.length > 1 ? "s" : ""
    } pour "${searchTerm}"`
  );
}

/**
 * Gère la recherche en temps réel
 */
function handleSearch() {
  const searchInput = document.getElementById("searchInput");
  const clearBtn = document.getElementById("clearSearch");
  const searchTerm = searchInput.value;

  // Afficher/masquer le bouton d'effacement
  if (searchTerm) {
    clearBtn.classList.add("visible");
  } else {
    clearBtn.classList.remove("visible");
  }

  // Filtrer les ports
  filterPorts(searchTerm);

  // Afficher les résultats
  renderTable();

  // Gérer l'état vide
  const emptyState = document.getElementById("emptyState");
  if (filteredPorts.length === 0 && allPorts.length > 0) {
    emptyState.style.display = "flex";
  } else {
    emptyState.style.display = "none";
  }
}

/**
 * Gère le tri des colonnes
 * @param {string} column - Nom de la colonne à trier
 */
function handleSort(column) {
  // Inverser la direction si on clique sur la même colonne
  if (sortColumn === column) {
    sortDirection = sortDirection === "asc" ? "desc" : "asc";
  } else {
    sortColumn = column;
    sortDirection = "asc";
  }

  // Mettre à jour l'affichage des en-têtes
  document.querySelectorAll(".sortable").forEach((header) => {
    header.classList.remove("sorted", "asc", "desc");
  });

  const activeHeader = document.querySelector(`[data-sort="${column}"]`);
  activeHeader.classList.add("sorted", sortDirection);

  // Trier et afficher
  sortPorts();
  renderTable();
}

/**
 * Trie les ports filtrés selon la colonne et la direction actuelles
 */
function sortPorts() {
  filteredPorts.sort((a, b) => {
    let aVal, bVal;

    if (sortColumn === "port") {
      aVal = typeof a.port === "number" ? a.port : parseInt(a.port) || 0;
      bVal = typeof b.port === "number" ? b.port : parseInt(b.port) || 0;
    } else if (sortColumn === "process") {
      aVal = (a.processName || "").toLowerCase();
      bVal = (b.processName || "").toLowerCase();
    }

    if (aVal < bVal) return sortDirection === "asc" ? -1 : 1;
    if (aVal > bVal) return sortDirection === "asc" ? 1 : -1;
    return 0;
  });
}

/**
 * Affiche les ports dans le tableau
 */
function renderTable() {
  const tableBody = document.getElementById("portsTableBody");
  tableBody.innerHTML = "";

  filteredPorts.forEach((port) => {
    const row = createTableRow(port);
    tableBody.appendChild(row);
  });

  // Mettre à jour le compteur
  const portCount = document.getElementById("portCount");
  portCount.textContent = `${filteredPorts.length} port${
    filteredPorts.length > 1 ? "s" : ""
  }`;
}

/**
 * Crée une ligne de tableau pour un port
 * @param {Object} port - Données du port
 * @returns {HTMLElement} - Élément TR
 */
function createTableRow(port) {
  const tr = document.createElement("tr");
  tr.setAttribute("data-port", port.port);

  // Colonne Port
  const portCell = document.createElement("td");
  portCell.className = "col-port";
  portCell.innerHTML = `<span class="port-number">${port.port}</span>`;
  tr.appendChild(portCell);

  // Colonne Protocole
  const protocolCell = document.createElement("td");
  protocolCell.className = "col-protocol";
  protocolCell.innerHTML = `<span class="protocol-badge">${port.protocol}</span>`;
  tr.appendChild(protocolCell);

  // Colonne Adresse
  const addressCell = document.createElement("td");
  addressCell.className = "col-address";
  addressCell.innerHTML = `<span class="address-cell">${port.address}</span>`;
  tr.appendChild(addressCell);

  // Colonne Application
  const processCell = document.createElement("td");
  processCell.className = "col-process";
  const processInitial = (port.processName || "U").charAt(0).toUpperCase();
  processCell.innerHTML = `
    <div class="process-name">
      <span class="process-icon">${processInitial}</span>
      <span>${port.processName || "Inconnu"}</span>
    </div>
  `;
  tr.appendChild(processCell);

  // Colonne PID
  const pidCell = document.createElement("td");
  pidCell.className = "col-pid";
  pidCell.innerHTML = `<span class="pid-cell">${port.pid}</span>`;
  tr.appendChild(pidCell);

  // Colonne Actions
  const actionsCell = document.createElement("td");
  actionsCell.className = "col-actions actions-cell";

  const killBtn = document.createElement("button");
  killBtn.className = "btn btn-danger btn-small";
  killBtn.innerHTML = `
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
      <path d="M18 6L6 18M6 6L18 18" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
    </svg>
    Kill
  `;
  killBtn.addEventListener("click", () => showKillModal(port));

  actionsCell.appendChild(killBtn);
  tr.appendChild(actionsCell);

  return tr;
}

/**
 * Affiche le modal de confirmation de terminaison de processus
 * @param {Object} port - Données du port
 */
function showKillModal(port) {
  currentKillPid = port.pid;
  currentKillPort = port.port;
  currentKillProcess = port.processName || "Inconnu";

  document.getElementById("modalPort").textContent = port.port;
  document.getElementById("modalPid").textContent = port.pid;
  document.getElementById("modalProcess").textContent = currentKillProcess;

  const modal = document.getElementById("confirmModal");
  modal.style.display = "flex";
}

/**
 * Ferme le modal de confirmation
 */
function closeModal() {
  const modal = document.getElementById("confirmModal");
  modal.style.display = "none";
  currentKillPid = null;
  currentKillPort = null;
  currentKillProcess = null;
}

/**
 * Confirme et exécute la terminaison du processus
 */
async function confirmKillProcess() {
  if (!currentKillPid) return;

  const confirmBtn = document.getElementById("confirmKillBtn");
  confirmBtn.disabled = true;
  confirmBtn.innerHTML =
    '<span class="spinner" style="width: 16px; height: 16px; border-width: 2px;"></span> Terminaison...';

  try {
    const result = await window.electronAPI.killProcess(currentKillPid);

    if (result.success) {
      showToast(
        `Processus ${currentKillProcess} (PID: ${currentKillPid}) terminé avec succès`,
        "success"
      );

      // Actualiser la liste après 1 seconde
      setTimeout(loadPorts, 1000);
    } else {
      showToast(`Erreur: ${result.message}`, "error");
    }
  } catch (error) {
    console.error("Erreur lors de la terminaison:", error);
    showToast("Erreur lors de la terminaison du processus", "error");
  } finally {
    confirmBtn.disabled = false;
    confirmBtn.innerHTML = "Terminer le processus";
    closeModal();
  }
}

/**
 * Charge les favoris depuis localStorage
 */
function loadFavorites() {
  const saved = localStorage.getItem("favoritePorts");
  if (saved) {
    try {
      favoritePorts = JSON.parse(saved);
    } catch (error) {
      console.error("Erreur lors du chargement des favoris:", error);
      favoritePorts = [];
    }
  }
}

/**
 * Sauvegarde les favoris dans localStorage
 */
function saveFavoritesToStorage() {
  localStorage.setItem("favoritePorts", JSON.stringify(favoritePorts));
}

/**
 * Change d'onglet et affiche la page correspondante
 */
function switchTab(tabName) {
  currentPage = tabName;

  // Mettre à jour les onglets actifs
  document.querySelectorAll(".tab").forEach((tab) => {
    if (tab.dataset.tab === tabName) {
      tab.classList.add("active");
    } else {
      tab.classList.remove("active");
    }
  });

  // Cacher toutes les pages
  document.querySelectorAll(".page").forEach((page) => {
    page.classList.remove("active");
  });

  // Afficher la page demandée
  if (tabName === "favorites") {
    document.getElementById("favoritesPage").classList.add("active");
    renderFavorites();
  } else {
    document.getElementById("portsPage").classList.add("active");
  }
}

/**
 * Affiche une page spécifique (pour compatibilité)
 */
function showPage(pageName) {
  switchTab(pageName);
}

/**
 * Affiche le modal d'ajout de favori
 */
function showAddFavoriteModal() {
  document.getElementById("favoritePort").value = "";
  document.getElementById("favoriteName").value = "";
  document.getElementById("favoriteDescription").value = "";
  document.getElementById("addFavoriteModal").style.display = "flex";
}

/**
 * Ferme le modal d'ajout de favori
 */
function closeAddFavoriteModal() {
  document.getElementById("addFavoriteModal").style.display = "none";
}

/**
 * Sauvegarde un nouveau favori
 */
function saveFavorite() {
  const port = parseInt(document.getElementById("favoritePort").value);
  const name = document.getElementById("favoriteName").value.trim();
  const description = document.getElementById("favoriteDescription").value.trim();

  if (!port || port < 1 || port > 65535) {
    showToast("Port invalide. Doit être entre 1 et 65535", "error");
    return;
  }

  if (!name) {
    showToast("Le nom est requis", "error");
    return;
  }

  // Vérifier si le port existe déjà
  if (favoritePorts.some((f) => f.port === port)) {
    showToast("Ce port est déjà dans vos favoris", "error");
    return;
  }

  // Ajouter le favori
  favoritePorts.push({ port, name, description: description || "" });
  saveFavoritesToStorage();
  renderFavorites();
  updateTabBadges();
  closeAddFavoriteModal();
  showToast(`Port ${port} ajouté aux favoris`, "success");
}

/**
 * Supprime un favori
 */
function deleteFavorite(port) {
  favoritePorts = favoritePorts.filter((f) => f.port !== port);
  saveFavoritesToStorage();
  renderFavorites();
  updateTabBadges();
  showToast(`Port ${port} retiré des favoris`, "success");
}

/**
 * Affiche les cartes de ports favoris
 */
function renderFavorites() {
  const container = document.getElementById("favoritesPorts");
  const emptyState = document.getElementById("emptyFavorites");

  container.innerHTML = "";

  if (favoritePorts.length === 0) {
    emptyState.style.display = "flex";
    return;
  }

  emptyState.style.display = "none";

  favoritePorts.forEach((favorite) => {
    const card = createFavoriteCard(favorite);
    container.appendChild(card);
  });
}

/**
 * Met à jour l'état des cartes de favoris
 */
function updateFavorites() {
  // Recréer les cartes pour mettre à jour les boutons Kill et les statuts
  if (currentPage === "favorites") {
    renderFavorites();
  }
}

/**
 * Crée une carte de favori
 * @param {Object} favorite - Données du favori
 * @returns {HTMLElement} - Élément de carte
 */
function createFavoriteCard(favorite) {
  const card = document.createElement("div");
  card.className = "favorite-card";
  card.setAttribute("data-favorite-port", favorite.port);

  const isActive = allPorts.some((p) => String(p.port) === String(favorite.port));
  const activePort = allPorts.find((p) => String(p.port) === String(favorite.port));

  card.innerHTML = `
    <div class="favorite-port">${favorite.port}</div>
    <div class="favorite-name">${favorite.name}</div>
    ${
      favorite.description
        ? `<div class="favorite-description">${favorite.description}</div>`
        : ""
    }
    <div class="favorite-actions">
      ${isActive ? `
        <button class="btn btn-danger btn-small kill-btn" data-pid="${activePort?.pid}" data-port="${favorite.port}" data-process="${activePort?.processName || 'Inconnu'}">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M18 6L6 18M6 6L18 18" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
          </svg>
          <span>Kill</span>
        </button>
      ` : ''}
      <button class="btn btn-secondary btn-small view-btn">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
          <circle cx="11" cy="11" r="8" stroke="currentColor" stroke-width="2"/>
          <path d="M21 21L16.65 16.65" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
        </svg>
        <span>Voir</span>
      </button>
      <button class="btn btn-danger btn-small delete-btn">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
          <path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <span>Suppr.</span>
      </button>
    </div>
  `;

  // Bouton pour kill le processus (si actif)
  const killBtn = card.querySelector(".kill-btn");
  if (killBtn) {
    killBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      const pid = killBtn.dataset.pid;
      const port = killBtn.dataset.port;
      const process = killBtn.dataset.process;
      showKillModal({ pid, port, processName: process });
    });
  }

  // Bouton pour voir le port
  const viewBtn = card.querySelector(".view-btn");
  viewBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    showPage("ports");
    const searchInput = document.getElementById("searchInput");
    searchInput.value = String(favorite.port);
    handleSearch();
  });

  // Bouton pour supprimer le favori
  const deleteBtn = card.querySelector(".delete-btn");
  deleteBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    deleteFavorite(favorite.port);
  });

  // Appliquer la classe active/inactive à la carte
  if (isActive) {
    card.classList.add("active");
    card.classList.remove("inactive");
  } else {
    card.classList.remove("active");
    card.classList.add("inactive");
  }

  return card;
}

/**
 * Met à jour les badges des onglets
 */
function updateTabBadges() {
  const portsTabBadge = document.getElementById("portsTabBadge");
  const favoritesTabBadge = document.getElementById("favoritesTabBadge");

  if (portsTabBadge) {
    portsTabBadge.textContent = allPorts.length;
  }

  if (favoritesTabBadge) {
    favoritesTabBadge.textContent = favoritePorts.length;
  }
}

/**
 * Met à jour le statut de filtre
 * @param {string} status - Message de statut
 */
function updateFilterStatus(status) {
  const filterStatus = document.getElementById("filterStatus");
  filterStatus.textContent = status;
}

/**
 * Affiche une notification toast
 * @param {string} message - Message à afficher
 * @param {string} type - Type de notification (success, error, info)
 */
function showToast(message, type = "info") {
  const container = document.getElementById("toastContainer");

  const toast = document.createElement("div");
  toast.className = `toast toast-${type}`;

  let icon;
  let title;

  switch (type) {
    case "success":
      icon = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                <path d="M20 6L9 17L4 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>`;
      title = "Succès";
      break;
    case "error":
      icon = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2"/>
                <path d="M12 8V12M12 16H12.01" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
              </svg>`;
      title = "Erreur";
      break;
    default:
      icon = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2"/>
                <path d="M12 16V12M12 8H12.01" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
              </svg>`;
      title = "Information";
  }

  toast.innerHTML = `
    <div class="toast-icon" style="color: var(--${type})">${icon}</div>
    <div class="toast-content">
      <div class="toast-title">${title}</div>
      <div class="toast-message">${message}</div>
    </div>
    <button class="toast-close">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
        <path d="M18 6L6 18M6 6L18 18" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
      </svg>
    </button>
  `;

  const closeBtn = toast.querySelector(".toast-close");
  closeBtn.addEventListener("click", () => {
    toast.style.animation = "slideIn 0.3s ease-out reverse";
    setTimeout(() => toast.remove(), 300);
  });

  container.appendChild(toast);

  // Auto-fermeture après 5 secondes
  setTimeout(() => {
    if (toast.parentElement) {
      toast.style.animation = "slideIn 0.3s ease-out reverse";
      setTimeout(() => toast.remove(), 300);
    }
  }, 5000);
}

// Exposer les fonctions nécessaires globalement pour le HTML
window.closeModal = closeModal;
window.closeAddFavoriteModal = closeAddFavoriteModal;
