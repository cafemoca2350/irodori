(function() {
"use strict";
console.log("[iRodoRi] アプリケーションを起動中...");

// Tauri 2.0 API Detection
let isTauri = false;
let invoke = async (cmd, args) => { console.log(`[Mock] ${cmd} 実行:`, args); return null; };
let listen = () => {};
let appWindow = { minimize: () => {}, close: () => {}, hide: () => {} };
let globalShortcut = null;

try {
  if (typeof window !== 'undefined' && window.__TAURI__) {
    isTauri = true;
    const tauri = window.__TAURI__;
    if (tauri.core) invoke = tauri.core.invoke;
    if (tauri.event) listen = tauri.event.listen;
    if (tauri.window) appWindow = tauri.window.getCurrentWindow();
    
    // In Tauri 2.0, plugins are often under window.__TAURI__['plugin-name']
    // Or we can just use invoke to register if we implement it, but let's try the direct approach
    console.log("[iRodoRi] Tauri API 取得完了");
  }
} catch (e) {
  console.error("[iRodoRi] 初期化エラー:", e);
}

// Diagnostic tests
(async () => {
  try {
    const nvapiResult = await invoke('test_nvapi', {});
    console.log("[iRodoRi] NVAPI テスト:", nvapiResult);
  } catch (e) {
    console.warn("[iRodoRi] NVAPI テスト失敗:", e);
  }
  try {
    const gammaResult = await invoke('test_gamma', {});
    console.log("[iRodoRi] ガンマテスト:", gammaResult);
  } catch (e) {
    console.warn("[iRodoRi] ガンマテスト失敗:", e);
  }
})();

// Elements
const brightnessSlider = document.getElementById('brightness-slider');
const contrastSlider = document.getElementById('contrast-slider');
const gammaSlider = document.getElementById('gamma-slider');
const vibranceSlider = document.getElementById('vibrance-slider');
const hueSlider = document.getElementById('hue-slider');

const brightnessVal = document.getElementById('brightness-val');
const contrastVal = document.getElementById('contrast-val');
const gammaVal = document.getElementById('gamma-val');
const vibranceVal = document.getElementById('vibrance-val');
const hueVal = document.getElementById('hue-val');

const presetsList = document.getElementById('presets-list');
const addPresetBtn = document.getElementById('add-preset-btn');

// Titlebar controls
document.getElementById('titlebar').addEventListener('mousedown', (e) => {
  if (e.target.closest('.titlebar-button')) return;
  if (appWindow.startDragging) appWindow.startDragging().catch(() => {});
});

document.getElementById('titlebar-minimize').addEventListener('click', async (e) => {
  e.stopPropagation();
  try { await appWindow.hide(); } catch (err) {}
});

document.getElementById('titlebar-close').addEventListener('click', async (e) => {
  e.stopPropagation();
  try { await appWindow.hide(); } catch (err) {}
});

// Update logic
function updateActivePresetValue(key, val) {
  const p = presets.find(item => item.id === activePresetId);
  if (p) {
    p[key] = (key === 'gamma' || key === 'hue' || key === 'vibrance') ? parseFloat(val) : parseInt(val);
    savePresets();
  }
}

let pendingApply = false;
function applyAllSettings() {
  if (pendingApply) return;
  pendingApply = true;
  requestAnimationFrame(async () => {
    pendingApply = false;
    try {
      await invoke('apply_color_settings', {
        settings: {
          brightness: parseFloat(brightnessSlider.value),
          contrast: parseFloat(contrastSlider.value),
          gamma: parseFloat(gammaSlider.value)
        }
      });
    } catch (e) {}
  });
}

let pendingEffect = false;
function applyColorEffect() {
  if (pendingEffect) return;
  pendingEffect = true;
  requestAnimationFrame(async () => {
    pendingEffect = false;
    try {
      await invoke('apply_color_effect', {
        effect: {
          saturation: 100.0,
          hue: parseFloat(hueSlider.value)
        }
      });
    } catch (e) {
      console.error('[iRodoRi] 色相の適用に失敗:', e);
    }
  });
}

// Apply NVAPI Digital Vibrance (saturation)
let pendingVibrance = false;
function applyVibrance() {
  if (pendingVibrance) return;
  pendingVibrance = true;
  requestAnimationFrame(async () => {
    pendingVibrance = false;
    try {
      const result = await invoke('apply_vibrance', {
        settings: {
          level: parseInt(vibranceSlider.value)
        }
      });
      console.log('[iRodoRi] Digital Vibrance:', result);
    } catch (e) {
      console.error('[iRodoRi] Digital Vibrance の適用に失敗:', e);
    }
  });
}

function updateBrightness(val, skipSlider = false) {
  brightnessVal.textContent = `${val}%`;
  if (!skipSlider) brightnessSlider.value = val;
  updateActivePresetValue('brightness', val);
  applyAllSettings();
}

function updateContrast(val, skipSlider = false) {
  contrastVal.textContent = `${val}%`;
  if (!skipSlider) contrastSlider.value = val;
  updateActivePresetValue('contrast', val);
  applyAllSettings();
}

function updateGamma(val, skipSlider = false) {
  gammaVal.textContent = parseFloat(val).toFixed(2);
  if (!skipSlider) gammaSlider.value = val;
  updateActivePresetValue('gamma', val);
  applyAllSettings();
}

function updateVibrance(val, skipSlider = false) {
  vibranceVal.textContent = `${val}%`;
  if (!skipSlider) vibranceSlider.value = val;
  updateActivePresetValue('vibrance', val);
  applyVibrance();
}

function updateHue(val, skipSlider = false) {
  hueVal.textContent = `${val}°`;
  if (!skipSlider) hueSlider.value = val;
  updateActivePresetValue('hue', val);
  applyColorEffect();
}

// Sliders
brightnessSlider.addEventListener('input', (e) => updateBrightness(e.target.value, true));
contrastSlider.addEventListener('input', (e) => updateContrast(e.target.value, true));
gammaSlider.addEventListener('input', (e) => updateGamma(e.target.value, true));
vibranceSlider.addEventListener('input', (e) => updateVibrance(e.target.value, true));
hueSlider.addEventListener('input', (e) => updateHue(e.target.value, true));

// Preset Management
let presets = JSON.parse(localStorage.getItem('irodori-presets')) || [
  { id: 'p1', name: '標準', brightness: 50, contrast: 50, gamma: 1.0, vibrance: 50, hue: 0, shortcut: null },
  { id: 'p2', name: 'ゲーム', brightness: 55, contrast: 60, gamma: 1.1, vibrance: 70, hue: 0, shortcut: 'CommandOrControl+Alt+G' },
  { id: 'p3', name: '映画', brightness: 45, contrast: 55, gamma: 0.9, vibrance: 50, hue: 0, shortcut: 'CommandOrControl+Alt+M' }
];

let activePresetId = localStorage.getItem('irodori-active-preset') || 'p1';
let recordingShortcutId = null;

function savePresets() {
  localStorage.setItem('irodori-presets', JSON.stringify(presets));
  // Sync global shortcuts whenever presets change
  syncGlobalShortcuts();
}

async function syncGlobalShortcuts() {
  if (!isTauri) return;
  try {
    // We'll use a custom command or the plugin API to register
    // For simplicity, let's assume we use the plugin's register API
    // but first unregister all to avoid conflicts
    const shortcutsPlugin = window.__TAURI__.globalShortcut;
    if (shortcutsPlugin) {
      await shortcutsPlugin.unregisterAll();
      for (const p of presets) {
        if (p.shortcut) {
          try {
            await shortcutsPlugin.register(p.shortcut, (shortcut) => {
              console.log(`Global shortcut triggered: ${shortcut}`);
              applyPreset(p.id);
            });
          } catch (err) {
            console.error(`Failed to register shortcut ${p.shortcut}:`, err);
          }
        }
      }
    }
  } catch (e) {
    console.error("Global shortcut sync failed:", e);
  }
}

function renderPresets() {
  presetsList.innerHTML = '';
  presets.forEach(p => {
    const card = document.createElement('div');
    card.className = `preset-hero ${p.id === activePresetId ? 'active' : ''}`;
    card.dataset.id = p.id;
    
    const formatShortcut = (s) => {
      if (!s) return 'ショートカット未設定';
      return s.replace('CommandOrControl', 'Ctrl').replace('Alt', 'Alt').replace('Shift', 'Shift').split('+').join(' + ');
    };

    const shortcutText = recordingShortcutId === p.id ? '入力待ち...' : (p.shortcut ? formatShortcut(p.shortcut) : 'ショートカット未設定');

    card.innerHTML = `
      <div class="hero-info">
        <div class="hero-name-container">
          <div class="hero-name" id="name-${p.id}">${p.name}</div>
          <div class="hero-actions">
            <button class="shortcut-btn ${p.shortcut ? 'has-shortcut' : ''}" title="ショートカットキーを設定">${shortcutText}</button>
            <button class="edit-name-btn">名前変更</button>
            ${p.id !== 'p1' ? `<button class="delete-preset-btn">削除</button>` : ''}
          </div>
        </div>
        ${p.id === activePresetId ? `
          <div class="advanced-content">
            <div class="control-group">
              <div class="slider-container">
                <div class="slider-header"><span>明るさ</span><span class="slider-value">${p.brightness}%</span></div>
              </div>
              <div class="slider-container">
                <div class="slider-header"><span>コントラスト</span><span class="slider-value">${p.contrast}%</span></div>
              </div>
              <div class="slider-container">
                <div class="slider-header"><span>ガンマ</span><span class="slider-value">${p.gamma.toFixed(2)}</span></div>
              </div>
            </div>
            <div class="control-group">
              <div class="slider-container">
                <div class="slider-header"><span>デジタルバイブランス</span><span class="slider-value">${p.vibrance}%</span></div>
              </div>
              <div class="slider-container">
                <div class="slider-header"><span>色相</span><span class="slider-value">${p.hue}°</span></div>
              </div>
            </div>
          </div>
        ` : ''}
      </div>
    `;

    card.addEventListener('click', (e) => {
      if (e.target.closest('button') || e.target.closest('[contenteditable="true"]')) return;
      applyPreset(p.id);
    });

    const shortcutBtn = card.querySelector('.shortcut-btn');
    shortcutBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      recordingShortcutId = (recordingShortcutId === p.id) ? null : p.id;
      renderPresets();
    });

    const editBtn = card.querySelector('.edit-name-btn');
    const nameEl = card.querySelector('.hero-name');
    editBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      if (nameEl.contentEditable === "true") {
        nameEl.contentEditable = "false";
        editBtn.textContent = "名前変更";
        p.name = nameEl.textContent;
        savePresets();
      } else {
        nameEl.contentEditable = "true";
        nameEl.focus();
        editBtn.textContent = "決定";
        const save = () => {
          nameEl.contentEditable = "false";
          editBtn.textContent = "名前変更";
          p.name = nameEl.textContent;
          savePresets();
        };
        nameEl.onblur = save;
        nameEl.onkeydown = (ke) => { if (ke.key === 'Enter') { ke.preventDefault(); save(); } };
      }
    });

    if (p.id !== 'p1') {
      const deleteBtn = card.querySelector('.delete-preset-btn');
      deleteBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        if (confirm(`プリセット「${p.name}」を削除しますか？`)) {
          presets = presets.filter(item => item.id !== p.id);
          if (activePresetId === p.id) activePresetId = 'p1';
          savePresets();
          renderPresets();
        }
      });
    }

    presetsList.appendChild(card);

    if (p.id === activePresetId) {
      const advancedSection = document.getElementById('advanced-mode');
      card.appendChild(advancedSection);
      advancedSection.classList.remove('collapsed');
    }
  });
}

// Local keyboard listener for recording shortcuts
window.addEventListener('keydown', (e) => {
  if (recordingShortcutId) {
    e.preventDefault();
    const p = presets.find(item => item.id === recordingShortcutId);
    if (p) {
      if (e.key === 'Escape') {
        p.shortcut = null;
        savePresets();
        recordingShortcutId = null;
        renderPresets();
        return;
      }

      if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) return;

      const mods = [];
      if (e.ctrlKey || e.metaKey) mods.push('CommandOrControl');
      if (e.altKey) mods.push('Alt');
      if (e.shiftKey) mods.push('Shift');

      const mainKey = e.key.toUpperCase();
      p.shortcut = mods.length > 0 ? `${mods.join('+')}+${mainKey}` : mainKey;
      
      savePresets();
      recordingShortcutId = null;
      renderPresets();
    }
  }
});

function applyPreset(id) {
  const p = presets.find(item => item.id === id);
  if (!p) return;

  activePresetId = id;
  localStorage.setItem('irodori-active-preset', id);

  updateBrightness(p.brightness);
  updateContrast(p.contrast);
  updateGamma(p.gamma);
  updateVibrance(p.vibrance);
  updateHue(p.hue);

  renderPresets();
}

addPresetBtn.addEventListener('click', () => {
  const newId = 'p' + Date.now();
  presets.push({
    id: newId,
    name: 'カスタム設定',
    brightness: parseInt(brightnessSlider.value),
    contrast: parseInt(contrastSlider.value),
    gamma: parseFloat(gammaSlider.value),
    vibrance: parseInt(vibranceSlider.value),
    hue: parseInt(hueSlider.value),
    shortcut: null
  });
  savePresets();
  renderPresets();
  applyPreset(newId);
});

// Initialization
(async () => {
  renderPresets();
  applyPreset(activePresetId);
  syncGlobalShortcuts();

  // Listen for global shortcut events from Rust (backup mechanism)
  if (listen) {
    listen('global-shortcut', (event) => {
      // Find preset with this shortcut and apply it
      const shortcutStr = event.payload; // e.g., "Shortcut { ... }"
      // This is complex to parse, so the direct JS API registration in syncGlobalShortcuts is better.
    });
  }

  // Auto-start check
  const autostartToggle = document.getElementById('autostart-toggle');
  if (autostartToggle) {
    try {
      const enabled = await invoke('check_autostart', {});
      autostartToggle.checked = enabled;
      autostartToggle.addEventListener('change', async () => {
        try {
          if (autostartToggle.checked) await invoke('enable_autostart', {});
          else await invoke('disable_autostart', {});
        } catch (e) {
          autostartToggle.checked = !autostartToggle.checked;
        }
      });
    } catch (e) {}
  }
})();

})();
