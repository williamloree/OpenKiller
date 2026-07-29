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
let currentKillMode = "single"; // "single" ou "bulk"
let currentPage = "ports"; // "ports" ou "favorites"
let selectedPids = new Set();

// Ports favoris - chargés depuis localStorage
let favoritePorts = [];
let editingFavoritePort = null;

// Dernier relevé de l'état de la RAM système
let lastMemoryInfo = null;

// Bibliothèque de ports courants pour l'ajout rapide en favoris
const PORT_PRESETS = [
  { port: 3000, name: "Nuxt / Node dev" },
  { port: 5173, name: "Vite dev" },
  { port: 4200, name: "Angular dev" },
  { port: 8000, name: "Django dev" },
  { port: 5000, name: "Flask dev" },
  { port: 80, name: "Caddy / HTTP" },
  { port: 443, name: "Caddy / HTTPS" },
  { port: 8080, name: "HTTP alternatif" },
  { port: 5432, name: "PostgreSQL" },
  { port: 3306, name: "MySQL" },
  { port: 6379, name: "Redis" },
  { port: 27017, name: "MongoDB" },
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
  // Charger les favoris depuis localStorage
  loadFavorites();

  // Charger les ports au démarrage
  loadPorts();

  // Charger l'état de la RAM au démarrage
  loadMemoryInfo();

  // Configurer les écouteurs d'événements
  setupEventListeners();

  // Afficher les ports favoris
  renderFavorites();

  // Mettre à jour les badges
  updateTabBadges();

  // Actualiser automatiquement toutes les 10 secondes
  setInterval(() => {
    loadPorts();
    loadMemoryInfo();
  }, 10000);
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

  // Bouton de nettoyage RAM (ouvre la modale de confirmation)
  const cleanRamBtn = document.getElementById("cleanRamBtn");
  cleanRamBtn.addEventListener("click", showCleanRamModal);

  document.getElementById("cleanRamModalCloseBtn").addEventListener("click", closeCleanRamModal);
  document.getElementById("cleanRamModalCancelBtn").addEventListener("click", closeCleanRamModal);
  document.getElementById("confirmCleanRamBtn").addEventListener("click", () => {
    closeCleanRamModal();
    performCleanRam();
  });

  const cleanRamModal = document.getElementById("cleanRamModal");
  cleanRamModal.addEventListener("click", (e) => {
    if (e.target === cleanRamModal) {
      closeCleanRamModal();
    }
  });

  // Champ de recherche
  const searchInput = document.getElementById("searchInput");
  searchInput.addEventListener("input", handleSearch);

  // Filtre par protocole
  const protocolFilter = document.getElementById("protocolFilter");
  protocolFilter.addEventListener("change", handleSearch);

  // Export CSV / JSON
  document.getElementById("exportCsvBtn").addEventListener("click", () => exportPorts("csv"));
  document.getElementById("exportJsonBtn").addEventListener("click", () => exportPorts("json"));

  // Sélection multiple
  const selectAllCheckbox = document.getElementById("selectAllCheckbox");
  selectAllCheckbox.addEventListener("change", () => {
    const checked = selectAllCheckbox.checked;
    filteredPorts.forEach((port) => {
      const key = String(port.pid);
      if (checked) {
        selectedPids.add(key);
      } else {
        selectedPids.delete(key);
      }
    });
    renderTable();
  });

  document.getElementById("bulkClearBtn").addEventListener("click", () => {
    selectedPids.clear();
    renderTable();
  });

  document.getElementById("bulkKillBtn").addEventListener("click", () => {
    if (selectedPids.size === 0) return;
    showBulkKillModal();
  });

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

  // Fermeture des modals (croix et annuler)
  document.getElementById("confirmModalCloseBtn").addEventListener("click", closeModal);
  document.getElementById("confirmModalCancelBtn").addEventListener("click", closeModal);
  document.getElementById("addFavoriteModalCloseBtn").addEventListener("click", closeAddFavoriteModal);
  document.getElementById("addFavoriteModalCancelBtn").addEventListener("click", closeAddFavoriteModal);

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
      closeCleanRamModal();
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

    // Retirer de la sélection les PID qui ne sont plus présents
    const currentPids = new Set(allPorts.map((p) => String(p.pid)));
    selectedPids.forEach((pid) => {
      if (!currentPids.has(pid)) selectedPids.delete(pid);
    });

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
  const protocolValue = document.getElementById("protocolFilter").value;

  filteredPorts = allPorts.filter((port) => {
    const matchesProtocol =
      protocolValue === "all" ||
      String(port.protocol || "").toLowerCase().includes(protocolValue);

    if (!matchesProtocol) return false;
    if (!term) return true;

    const portStr = String(port.port).toLowerCase();
    const processStr = (port.processName || "").toLowerCase();
    const addressStr = (port.address || "").toLowerCase();

    return (
      portStr.includes(term) ||
      processStr.includes(term) ||
      addressStr.includes(term)
    );
  });

  if (term) {
    updateFilterStatus(
      `${filteredPorts.length} résultat${
        filteredPorts.length > 1 ? "s" : ""
      } pour "${searchTerm}"`
    );
  } else {
    updateFilterStatus("");
  }
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

  updateBulkActionBar();
  updateSelectAllCheckboxState();
}

/**
 * Crée une ligne de tableau pour un port
 * @param {Object} port - Données du port
 * @returns {HTMLElement} - Élément TR
 */
function createTableRow(port) {
  const tr = document.createElement("tr");
  tr.setAttribute("data-port", port.port);

  // Colonne Sélection
  const checkboxCell = document.createElement("td");
  checkboxCell.className = "col-checkbox";
  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  checkbox.className = "row-checkbox";
  checkbox.checked = selectedPids.has(String(port.pid));
  checkbox.addEventListener("change", () => {
    togglePidSelection(port.pid, checkbox.checked);
  });
  checkboxCell.appendChild(checkbox);
  tr.appendChild(checkboxCell);

  // Colonne Port
  const portCell = document.createElement("td");
  portCell.className = "col-port";
  const protocolDotClass = String(port.protocol || "").toLowerCase().includes("udp")
    ? "dot-udp"
    : "dot-tcp";
  portCell.innerHTML = `<span class="port-dot ${protocolDotClass}"></span><span class="port-number">${port.port}</span>`;
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
  const exePathTitle = port.exePath && port.exePath !== "N/A" ? port.exePath : "Chemin inconnu";
  processCell.innerHTML = `
    <div class="process-name" title="${exePathTitle.replace(/"/g, "&quot;")}">
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

  // Colonne RAM
  const ramCell = document.createElement("td");
  ramCell.className = "col-ram";
  ramCell.innerHTML = `<span class="ram-cell">${formatMemory(port.memoryMB)}</span>`;
  tr.appendChild(ramCell);

  // Colonne Actions
  const actionsCell = document.createElement("td");
  actionsCell.className = "col-actions actions-cell";

  const killBtn = document.createElement("button");
  killBtn.className = "btn btn-danger btn-small";
  killBtn.innerHTML = `
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
      <path d="M12 2v8" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
      <path d="M6.5 6.5a8 8 0 1 0 11 0" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
    </svg>
    Kill
  `;
  killBtn.addEventListener("click", () => showKillModal(port));

  const actionsInner = document.createElement("div");
  actionsInner.className = "actions-cell-inner";
  actionsInner.appendChild(killBtn);
  actionsCell.appendChild(actionsInner);
  tr.appendChild(actionsCell);

  return tr;
}

/**
 * Affiche le modal de confirmation de terminaison de processus
 * @param {Object} port - Données du port
 */
function showKillModal(port) {
  currentKillMode = "single";
  currentKillPid = port.pid;
  currentKillPort = port.port;
  currentKillProcess = port.processName || "Inconnu";

  document.getElementById("modalMessage").textContent =
    "Êtes-vous sûr de vouloir terminer ce processus ?";
  document.getElementById("modalPort").textContent = port.port;
  document.getElementById("modalPid").textContent = port.pid;
  document.getElementById("modalProcess").textContent = currentKillProcess;

  const modal = document.getElementById("confirmModal");
  modal.style.display = "flex";
}

/**
 * Affiche le modal de confirmation pour la terminaison groupée
 */
function showBulkKillModal() {
  currentKillMode = "bulk";

  document.getElementById("modalMessage").textContent =
    `Êtes-vous sûr de vouloir terminer ${selectedPids.size} processus ?`;
  document.getElementById("modalPort").textContent = "-";
  document.getElementById("modalPid").textContent = Array.from(selectedPids).join(", ");
  document.getElementById("modalProcess").textContent = `${selectedPids.size} processus sélectionnés`;

  const modal = document.getElementById("confirmModal");
  modal.style.display = "flex";
}

/**
 * Ferme le modal de confirmation
 */
function closeModal() {
  const modal = document.getElementById("confirmModal");
  modal.style.display = "none";
  currentKillMode = "single";
  currentKillPid = null;
  currentKillPort = null;
  currentKillProcess = null;
}

/**
 * Confirme et exécute la terminaison du processus (simple ou groupée)
 */
async function confirmKillProcess() {
  if (currentKillMode === "bulk") {
    await executeBulkKill();
    return;
  }

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
 * Termine tous les processus sélectionnés
 */
async function executeBulkKill() {
  const pids = Array.from(selectedPids);
  const confirmBtn = document.getElementById("confirmKillBtn");
  confirmBtn.disabled = true;
  confirmBtn.innerHTML =
    '<span class="spinner" style="width: 16px; height: 16px; border-width: 2px;"></span> Terminaison...';

  try {
    const results = await window.electronAPI.killProcesses(pids);
    const successCount = results.filter((r) => r.success).length;
    const failCount = results.length - successCount;

    if (successCount > 0) {
      showToast(
        `${successCount} processus terminé${successCount > 1 ? "s" : ""} avec succès`,
        "success"
      );
    }
    if (failCount > 0) {
      showToast(
        `${failCount} processus n'ont pas pu être terminés`,
        "error"
      );
    }

    selectedPids.clear();
    setTimeout(loadPorts, 1000);
  } catch (error) {
    console.error("Erreur lors de la terminaison groupée:", error);
    showToast("Erreur lors de la terminaison des processus", "error");
  } finally {
    confirmBtn.disabled = false;
    confirmBtn.innerHTML = "Terminer le processus";
    closeModal();
  }
}

/**
 * Ajoute ou retire un PID de la sélection courante
 */
function togglePidSelection(pid, checked) {
  const key = String(pid);
  if (checked) {
    selectedPids.add(key);
  } else {
    selectedPids.delete(key);
  }
  updateBulkActionBar();
  updateSelectAllCheckboxState();
}

/**
 * Affiche/masque la barre d'actions groupées selon la sélection
 */
function updateBulkActionBar() {
  const bar = document.getElementById("bulkActionBar");
  const count = document.getElementById("bulkSelectionCount");

  if (selectedPids.size > 0) {
    bar.style.display = "flex";
    count.textContent = `${selectedPids.size} sélectionné${selectedPids.size > 1 ? "s" : ""}`;
  } else {
    bar.style.display = "none";
  }
}

/**
 * Met à jour l'état (coché/indéterminé) de la case "tout sélectionner"
 */
function updateSelectAllCheckboxState() {
  const selectAll = document.getElementById("selectAllCheckbox");
  if (!selectAll) return;

  const visiblePids = filteredPorts.map((p) => String(p.pid));
  const selectedVisible = visiblePids.filter((pid) => selectedPids.has(pid));

  selectAll.checked = visiblePids.length > 0 && selectedVisible.length === visiblePids.length;
  selectAll.indeterminate = selectedVisible.length > 0 && selectedVisible.length < visiblePids.length;
}

/**
 * Formate une valeur de mémoire en Go pour l'affichage
 */
function formatGB(valueGB) {
  return `${(valueGB || 0).toFixed(1)} Go`;
}

/**
 * Charge l'état de la RAM système et met à jour la jauge
 */
async function loadMemoryInfo() {
  try {
    lastMemoryInfo = await window.electronAPI.getMemoryInfo();
    renderMemoryMeter(lastMemoryInfo);
  } catch (error) {
    console.error("Erreur lors de la récupération de l'état de la RAM:", error);
  }
}

/**
 * Met à jour la jauge de RAM (total / utilisée / libérable)
 */
function renderMemoryMeter(info) {
  const usedFill = document.getElementById("ramMeterUsedFill");
  const freeableFill = document.getElementById("ramMeterFreeableFill");
  const usedLabel = document.getElementById("ramMeterUsedLabel");
  const freeableLabel = document.getElementById("ramMeterFreeableLabel");

  if (!info || !info.totalGB) {
    usedLabel.textContent = "RAM indisponible";
    freeableLabel.textContent = "";
    usedFill.style.width = "0%";
    freeableFill.style.width = "0%";
    return;
  }

  const usedPct = Math.min(100, (info.usedGB / info.totalGB) * 100);
  const freeablePct = Math.min(usedPct, (info.freeableGB / info.totalGB) * 100);
  const freeableLeft = Math.max(0, usedPct - freeablePct);

  usedFill.style.width = `${usedPct}%`;
  freeableFill.style.width = `${freeablePct}%`;
  freeableFill.style.left = `${freeableLeft}%`;

  usedLabel.textContent = `${formatGB(info.usedGB)} / ${formatGB(info.totalGB)}`;
  freeableLabel.textContent = `Libérable: ${formatGB(info.freeableGB)}`;
}

/**
 * Affiche la modale de confirmation du nettoyage RAM, préremplie avec le dernier relevé
 */
function showCleanRamModal() {
  const info = lastMemoryInfo || { totalGB: 0, usedGB: 0, freeableGB: 0 };

  document.getElementById("cleanModalTotal").textContent = formatGB(info.totalGB);
  document.getElementById("cleanModalUsed").textContent = formatGB(info.usedGB);
  document.getElementById("cleanModalFreeable").textContent = `~${formatGB(info.freeableGB)}`;
  document.getElementById("confirmCleanRamBtn").textContent = `Vider ~${formatGB(info.freeableGB)}`;

  document.getElementById("cleanRamModal").style.display = "flex";
}

/**
 * Ferme la modale de confirmation du nettoyage RAM
 */
function closeCleanRamModal() {
  document.getElementById("cleanRamModal").style.display = "none";
}

/**
 * Libère la RAM inutilisée par les applications (peut déclencher une demande d'élévation)
 */
async function performCleanRam() {
  const cleanRamBtn = document.getElementById("cleanRamBtn");
  const originalHtml = cleanRamBtn.innerHTML;

  cleanRamBtn.disabled = true;
  cleanRamBtn.innerHTML =
    '<span class="spinner" style="width: 14px; height: 14px; border-width: 2px;"></span><span>Nettoyage...</span>';

  try {
    const result = await window.electronAPI.cleanRam();
    showToast(result.message, result.success ? "success" : "error");
    if (result.success) {
      await loadMemoryInfo();
    }
  } catch (error) {
    console.error("Erreur lors du nettoyage de la RAM:", error);
    showToast("Erreur lors du nettoyage de la RAM", "error");
  } finally {
    cleanRamBtn.disabled = false;
    cleanRamBtn.innerHTML = originalHtml;
  }
}

/**
 * Formate une valeur de mémoire en Mo pour l'affichage
 */
function formatMemory(memoryMB) {
  return memoryMB === null || memoryMB === undefined ? "N/A" : `${memoryMB} Mo`;
}

/**
 * Exporte les ports affichés au format CSV ou JSON
 */
function exportPorts(format) {
  if (filteredPorts.length === 0) {
    showToast("Aucune donnée à exporter", "error");
    return;
  }

  let content, mimeType, filename;

  if (format === "json") {
    content = JSON.stringify(filteredPorts, null, 2);
    mimeType = "application/json";
    filename = `open-killer-ports-${Date.now()}.json`;
  } else {
    const headers = ["Port", "Protocole", "Adresse", "Application", "PID", "RAM (Mo)", "Chemin"];
    const rows = filteredPorts.map((p) => [
      p.port,
      p.protocol,
      p.address,
      p.processName,
      p.pid,
      p.memoryMB ?? "",
      p.exePath ?? "",
    ]);
    content = [headers, ...rows]
      .map((row) => row.map((cell) => `"${String(cell).replace(/"/g, '""')}"`).join(","))
      .join("\n");
    mimeType = "text/csv";
    filename = `open-killer-ports-${Date.now()}.csv`;
  }

  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);

  showToast(`Export ${format.toUpperCase()} réussi`, "success");
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
  editingFavoritePort = null;
  document.getElementById("addFavoriteModalTitle").textContent = "Ajouter un favori";
  document.getElementById("saveFavoriteBtn").textContent = "Ajouter";
  document.getElementById("favoritePort").value = "";
  document.getElementById("favoriteName").value = "";
  document.getElementById("favoriteDescription").value = "";
  renderPresetChips();
  document.getElementById("addFavoriteModal").style.display = "flex";
}

/**
 * Affiche le modal en mode édition, préempli avec le favori existant
 * @param {Object} favorite - Favori à modifier
 */
function showEditFavoriteModal(favorite) {
  editingFavoritePort = favorite.port;
  document.getElementById("addFavoriteModalTitle").textContent = "Modifier le favori";
  document.getElementById("saveFavoriteBtn").textContent = "Enregistrer";
  document.getElementById("favoritePort").value = favorite.port;
  document.getElementById("favoriteName").value = favorite.name;
  document.getElementById("favoriteDescription").value = favorite.description || "";
  document.getElementById("presetLibrary").style.display = "none";
  document.getElementById("presetDivider").style.display = "none";
  document.getElementById("addFavoriteModal").style.display = "flex";
}

/**
 * Affiche la bibliothèque de ports courants dans le modal d'ajout
 */
function renderPresetChips() {
  const container = document.getElementById("presetChips");
  const library = document.getElementById("presetLibrary");
  const divider = document.getElementById("presetDivider");
  container.innerHTML = "";

  const availablePresets = PORT_PRESETS.filter(
    (preset) => !favoritePorts.some((f) => f.port === preset.port)
  );

  if (availablePresets.length === 0) {
    library.style.display = "none";
    divider.style.display = "none";
    return;
  }

  library.style.display = "block";
  divider.style.display = "flex";

  availablePresets.forEach((preset) => {
    const chip = document.createElement("button");
    chip.type = "button";
    chip.className = "preset-chip";
    chip.innerHTML = `
      <span class="preset-chip-port">${preset.port}</span>
      <span class="preset-chip-name">${preset.name}</span>
    `;

    chip.addEventListener("click", () => {
      if (addFavoriteEntry(preset.port, preset.name, "")) {
        renderPresetChips();
      }
    });

    container.appendChild(chip);
  });
}

/**
 * Ferme le modal d'ajout de favori
 */
function closeAddFavoriteModal() {
  document.getElementById("addFavoriteModal").style.display = "none";
  editingFavoritePort = null;
}

/**
 * Valide et ajoute un favori (utilisé par le formulaire et la bibliothèque de presets)
 * @returns {boolean} true si le favori a été ajouté
 */
function addFavoriteEntry(port, name, description) {
  if (!port || port < 1 || port > 65535) {
    showToast("Port invalide. Doit être entre 1 et 65535", "error");
    return false;
  }

  if (!name) {
    showToast("Le nom est requis", "error");
    return false;
  }

  // Vérifier si le port existe déjà
  if (favoritePorts.some((f) => f.port === port)) {
    showToast("Ce port est déjà dans vos favoris", "error");
    return false;
  }

  favoritePorts.push({ port, name, description: description || "" });
  saveFavoritesToStorage();
  renderFavorites();
  updateTabBadges();
  showToast(`Port ${port} ajouté aux favoris`, "success");
  return true;
}

/**
 * Valide et applique la modification d'un favori existant
 * @returns {boolean} true si le favori a été mis à jour
 */
function updateFavoriteEntry(originalPort, newPort, name, description) {
  if (!newPort || newPort < 1 || newPort > 65535) {
    showToast("Port invalide. Doit être entre 1 et 65535", "error");
    return false;
  }

  if (!name) {
    showToast("Le nom est requis", "error");
    return false;
  }

  if (newPort !== originalPort && favoritePorts.some((f) => f.port === newPort)) {
    showToast("Ce port est déjà dans vos favoris", "error");
    return false;
  }

  const favorite = favoritePorts.find((f) => f.port === originalPort);
  if (!favorite) return false;

  favorite.port = newPort;
  favorite.name = name;
  favorite.description = description || "";
  saveFavoritesToStorage();
  renderFavorites();
  updateTabBadges();
  showToast(`Favori port ${newPort} mis à jour`, "success");
  return true;
}

/**
 * Sauvegarde le favori saisi dans le formulaire (ajout ou modification)
 */
function saveFavorite() {
  const port = parseInt(document.getElementById("favoritePort").value);
  const name = document.getElementById("favoriteName").value.trim();
  const description = document.getElementById("favoriteDescription").value.trim();

  const saved =
    editingFavoritePort !== null
      ? updateFavoriteEntry(editingFavoritePort, port, name, description)
      : addFavoriteEntry(port, name, description);

  if (saved) {
    closeAddFavoriteModal();
  }
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
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M12 2v8" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
            <path d="M6.5 6.5a8 8 0 1 0 11 0" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
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
      <button class="btn btn-secondary btn-small edit-btn">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
          <path d="M12 20h9" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
          <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5Z" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/>
        </svg>
        <span>Modifier</span>
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

  // Bouton pour modifier le favori
  const editBtn = card.querySelector(".edit-btn");
  editBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    showEditFavoriteModal(favorite);
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

  const railPortMeter = document.getElementById("railPortMeter");
  if (railPortMeter) {
    railPortMeter.textContent = allPorts.length;
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
