/**
 * Preload Script - Interface sécurisée entre le processus principal et le renderer
 * Expose uniquement les API nécessaires au renderer via contextBridge
 */

const { contextBridge, ipcRenderer } = require('electron');

/**
 * API exposée au renderer de manière sécurisée
 * Toutes les communications avec le processus principal passent par ces méthodes
 */
contextBridge.exposeInMainWorld('electronAPI', {
  /**
   * Récupère la liste des ports ouverts
   * @returns {Promise<Array>} Liste des ports avec leurs informations
   */
  getPorts: () => ipcRenderer.invoke('get-ports'),

  /**
   * Termine un processus par son PID
   * @param {string|number} pid - ID du processus à terminer
   * @returns {Promise<Object>} Résultat de l'opération
   */
  killProcess: (pid) => ipcRenderer.invoke('kill-process', pid),

  /**
   * Termine plusieurs processus par leurs PID
   * @param {Array<string|number>} pids - IDs des processus à terminer
   * @returns {Promise<Array<Object>>} Résultats de l'opération par PID
   */
  killProcesses: (pids) => ipcRenderer.invoke('kill-processes', pids),

  /**
   * Libère la RAM inutilisée par les applications
   * @returns {Promise<Object>} Résultat de l'opération
   */
  cleanRam: () => ipcRenderer.invoke('clean-ram'),

  /**
   * Récupère l'état de la RAM système (total, utilisée, libérable)
   * @returns {Promise<Object>} Informations mémoire en Go
   */
  getMemoryInfo: () => ipcRenderer.invoke('get-memory-info'),

  /**
   * Informations sur le système
   */
  platform: process.platform
});

// Afficher un message dans la console pour confirmer le chargement
console.log('Preload script chargé avec succès - API disponible');
