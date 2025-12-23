/**
 * Open Killer - Application Electron
 * Fichier principal pour la gestion de la fenêtre et des processus système
 */

const { app, BrowserWindow, ipcMain } = require("electron");
const path = require("path");
const { exec } = require("child_process");
const util = require("util");
const execPromise = util.promisify(exec);

let mainWindow;

/**
 * Crée la fenêtre principale de l'application
 */
function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    resizable: false,
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, "..", "preload", "preload.js"),
    },
    backgroundColor: "#1a1a1a",
    icon: path.join(__dirname, "..", "..", "assets", "icon.png"),
    autoHideMenuBar: true,
    show: false,
  });

  mainWindow.loadFile(path.join(__dirname, "..", "renderer", "index.html"));

  // Afficher la fenêtre une fois prête pour éviter le flash
  mainWindow.once("ready-to-show", () => {
    mainWindow.show();
  });

  // Ouvrir DevTools en mode développement
  if (process.argv.includes("--dev")) {
    mainWindow.webContents.openDevTools();
  }
}

/**
 * Récupère la liste des ports ouverts sur le système
 * Compatible Windows, macOS et Linux
 */
async function getOpenPorts() {
  const platform = process.platform;
  let command;

  try {
    if (platform === "win32") {
      // Commande pour Windows
      command = "netstat -ano | findstr LISTENING";
    } else if (platform === "darwin") {
      // Commande pour macOS
      command = "lsof -iTCP -sTCP:LISTEN -n -P";
    } else {
      // Commande pour Linux
      command = "ss -tlnp || netstat -tlnp";
    }

    const { stdout } = await execPromise(command);
    return parsePortData(stdout, platform);
  } catch (error) {
    console.error("Erreur lors de la récupération des ports:", error);
    return [];
  }
}

/**
 * Parse les données de ports selon le système d'exploitation
 * @param {string} data - Sortie brute de la commande système
 * @param {string} platform - Plateforme système (win32, darwin, linux)
 * @returns {Array} - Liste des ports avec leurs informations
 */
function parsePortData(data, platform) {
  const lines = data.split("\n").filter((line) => line.trim());
  const ports = [];
  const seenPorts = new Set();

  lines.forEach((line) => {
    try {
      let port, pid, protocol, address, processName;

      if (platform === "win32") {
        // Parse pour Windows: TCP 0.0.0.0:3000 0.0.0.0:0 LISTENING 1234
        const parts = line.trim().split(/\s+/);
        if (parts.length >= 5) {
          protocol = parts[0];
          const addressParts = parts[1].split(":");
          port = addressParts[addressParts.length - 1];
          address = parts[1];
          pid = parts[parts.length - 1];
          processName = "Processus système";
        }
      } else if (platform === "darwin") {
        // Parse pour macOS: node 1234 user 20u IPv4 0x... 0t0 TCP *:3000 (LISTEN)
        const parts = line.trim().split(/\s+/);
        if (parts.length >= 9) {
          processName = parts[0];
          pid = parts[1];
          const portInfo = parts[8].split(":");
          port = portInfo[portInfo.length - 1];
          protocol = parts[7];
          address = parts[8];
        }
      } else {
        // Parse pour Linux (ss ou netstat)
        if (line.includes("LISTEN")) {
          const parts = line.trim().split(/\s+/);
          if (parts.length >= 5) {
            protocol = parts[0];
            const addressParts = parts[4].split(":");
            port = addressParts[addressParts.length - 1];
            address = parts[4];

            // Extraire PID et nom du processus de la dernière colonne
            const lastCol = parts[parts.length - 1];
            const pidMatch = lastCol.match(/pid=(\d+)/);
            pid = pidMatch ? pidMatch[1] : "N/A";

            const nameMatch = lastCol.match(/users:\(\("([^"]+)"/);
            processName = nameMatch ? nameMatch[1] : "Processus système";
          }
        }
      }

      // Ajouter le port s'il est valide et non dupliqué
      if (port && !seenPorts.has(port)) {
        seenPorts.add(port);
        ports.push({
          port: parseInt(port) || port,
          pid: pid || "N/A",
          protocol: protocol || "TCP",
          address: address || "N/A",
          processName: processName || "Inconnu",
          timestamp: Date.now(),
        });
      }
    } catch (error) {
      // Ignorer les lignes mal formées
      console.warn("Ligne ignorée:", line);
    }
  });

  return ports.sort((a, b) => {
    const portA = typeof a.port === "number" ? a.port : parseInt(a.port) || 0;
    const portB = typeof b.port === "number" ? b.port : parseInt(b.port) || 0;
    return portA - portB;
  });
}

/**
 * Tue un processus par son PID
 * @param {string|number} pid - ID du processus à terminer
 */
async function killProcess(pid) {
  const platform = process.platform;
  let command;

  try {
    if (platform === "win32") {
      command = `taskkill /F /PID ${pid}`;
    } else {
      command = `kill -9 ${pid}`;
    }

    await execPromise(command);
    return { success: true, message: `Processus ${pid} terminé avec succès` };
  } catch (error) {
    console.error(`Erreur lors de la terminaison du processus ${pid}:`, error);
    return {
      success: false,
      message: `Échec de la terminaison du processus: ${error.message}`,
    };
  }
}

/**
 * Récupère le nom du processus pour un PID donné (amélioration)
 * @param {string|number} pid - ID du processus
 */
async function getProcessName(pid) {
  const platform = process.platform;
  let command;

  try {
    if (platform === "win32") {
      command = `tasklist /FI "PID eq ${pid}" /FO CSV /NH`;
    } else if (platform === "darwin") {
      command = `ps -p ${pid} -o comm=`;
    } else {
      command = `ps -p ${pid} -o comm=`;
    }

    const { stdout } = await execPromise(command);
    return stdout.trim().replace(/['"]/g, "").split(",")[0] || "Inconnu";
  } catch (error) {
    return "Inconnu";
  }
}

// Gestionnaires d'événements IPC
ipcMain.handle("get-ports", async () => {
  return await getOpenPorts();
});

ipcMain.handle("kill-process", async (event, pid) => {
  return await killProcess(pid);
});

ipcMain.handle("get-process-name", async (event, pid) => {
  return await getProcessName(pid);
});

// Événements de cycle de vie de l'application
app.whenReady().then(createWindow);

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});

app.on("activate", () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});

// Gestion des erreurs non capturées
process.on("uncaughtException", (error) => {
  console.error("Erreur non capturée:", error);
});
