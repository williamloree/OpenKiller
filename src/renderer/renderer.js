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

// Ports favoris configurables
const FAVORITE_PORTS = [
  { port: 3000, name: "Node.js Dev", description: "Serveur de développement" },
  { port: 8080, name: "HTTP Alt", description: "Serveur web alternatif" },
  { port: 5432, name: "PostgreSQL", description: "Base de données" },
  { port: 3306, name: "MySQL", description: "Base de données" },
  { port: 27017, name: "MongoDB", description: "Base de données NoSQL" },
  { port: 6379, name: "Redis", description: "Cache mémoire" },
];

// === Initialisation ===
document.addEventListener("DOMContentLoaded", () => {
  console.log("Application initialisée");
  initializeApp();
});

/**
 * Initialise l'application et tous les écouteurs d'événements
 */
function initializeApp() {
  // Charger les ports au démarrage
  loadPorts();

  // Configurer les écouteurs d'événements
  setupEventListeners();

  // Afficher les ports favoris
  renderFavorites();

  // Actualiser automatiquement toutes les 10 secondes
  setInterval(loadPorts, 10000);
}

/**
 * Configure tous les écouteurs d'événements de l'interface
 */
function setupEventListeners() {
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

  // Fermeture du modal avec la touche Échap
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      closeModal();
    }
  });

  // Clic en dehors du modal pour fermer
  const modal = document.getElementById("confirmModal");
  modal.addEventListener("click", (e) => {
    if (e.target === modal) {
      closeModal();
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
 * Affiche les cartes de ports favoris
 */
function renderFavorites() {
  const container = document.getElementById("favoritesPorts");
  container.innerHTML = "";

  FAVORITE_PORTS.forEach((favorite) => {
    const card = createFavoriteCard(favorite);
    container.appendChild(card);
  });
}

/**
 * Met à jour l'état des cartes de favoris
 */
function updateFavorites() {
  FAVORITE_PORTS.forEach((favorite) => {
    const card = document.querySelector(
      `[data-favorite-port="${favorite.port}"]`
    );
    if (!card) return;

    const isActive = allPorts.some(
      (p) => String(p.port) === String(favorite.port)
    );
    const statusEl = card.querySelector(".favorite-status");

    if (isActive) {
      card.classList.add("active");
      card.classList.remove("inactive");
      statusEl.className = "favorite-status status-active";
      statusEl.innerHTML = '<span class="status-dot"></span>Actif';
    } else {
      card.classList.remove("active");
      card.classList.add("inactive");
      statusEl.className = "favorite-status status-inactive";
      statusEl.innerHTML = '<span class="status-dot"></span>Inactif';
    }
  });
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

  card.innerHTML = `
    <div class="favorite-port">${favorite.port}</div>
    <div class="favorite-name">${favorite.name}</div>
    <span class="favorite-status status-inactive">
      <span class="status-dot"></span>
      Inactif
    </span>
  `;

  // Cliquer sur une carte filtre ce port
  card.addEventListener("click", () => {
    const searchInput = document.getElementById("searchInput");
    searchInput.value = String(favorite.port);
    handleSearch();
  });

  return card;
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
