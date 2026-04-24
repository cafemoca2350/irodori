(function() {
"use strict";
console.log("[iRodoRi] アプリケーションを起動中...");

// Tauri 2.0 API Detection - 完全防御型
let isTauri = false;
let invoke = async (cmd, args) => { console.log(`[Mock] ${cmd} 実行:`, args); return null; };
let listen = () => {};
let appWindow = { minimize: () => {}, close: () => {} };

try {
  if (typeof window !== 'undefined' && window.__TAURI__) {
    console.log("[iRodoRi] Tauri 環境を検出しました");
    isTauri = true;
    const tauri = window.__TAURI__;

    if (tauri.core && typeof tauri.core.invoke === 'function') {
      invoke = tauri.core.invoke;
    }
    if (tauri.event && typeof tauri.event.listen === 'function') {
      listen = tauri.event.listen;
    }
    if (tauri.window && typeof tauri.window.getCurrentWindow === 'function') {
      appWindow = tauri.window.getCurrentWindow();
    }
    console.log("[iRodoRi] Tauri API 取得完了");
  } else {
    console.log("[iRodoRi] ブラウザ環境（モックモード）で動作中");
  }
} catch (e) {
  console.error("[iRodoRi] 初期化エラー（UIは動作を続けます）:", e);
}

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
const toggleAdvanced = document.getElementById('toggle-advanced');
const advancedSection = document.getElementById('advanced-mode');

// Titlebar
document.getElementById('titlebar-minimize').addEventListener('click', () => appWindow.minimize());
document.getElementById('titlebar-close').addEventListener('click', () => appWindow.close());

// Toggle Advanced
toggleAdvanced.addEventListener('click', () => {
  advancedSection.classList.toggle('collapsed');
});

// Update logic
function updateActivePresetValue(key, val) {
  const p = presets.find(item => item.id === activePresetId);
  if (p) {
    p[key] = key === 'gamma' ? parseFloat(val) : parseInt(val);
    savePresets();
  }
}

async function updateBrightness(val, skipSlider = false) {
  brightnessVal.textContent = `${val}%`;
  if (!skipSlider) brightnessSlider.value = val;
  updateActivePresetValue('brightness', val);
  try {
    await invoke('set_monitor_brightness', { value: parseInt(val) });
  } catch (e) {
    console.error(e);
  }
}

async function updateContrast(val, skipSlider = false) {
  contrastVal.textContent = `${val}%`;
  if (!skipSlider) contrastSlider.value = val;
  updateActivePresetValue('contrast', val);
  try {
    await invoke('set_monitor_contrast', { value: parseInt(val) });
  } catch (e) {
    console.error(e);
  }
}

async function updateGamma(val, skipSlider = false) {
  gammaVal.textContent = parseFloat(val).toFixed(2);
  if (!skipSlider) gammaSlider.value = val;
  updateActivePresetValue('gamma', val);
  try {
    await invoke('set_gamma', { gamma: parseFloat(val) });
  } catch (e) {
    console.error(e);
  }
}

async function updateVibrance(val, skipSlider = false) {
  vibranceVal.textContent = `${val}%`;
  if (!skipSlider) vibranceSlider.value = val;
  updateActivePresetValue('vibrance', val);
  try {
    await invoke('set_digital_vibrance', { value: parseInt(val) });
  } catch (e) {
    console.error(e);
  }
}

async function updateHue(val, skipSlider = false) {
  hueVal.textContent = `${val}°`;
  if (!skipSlider) hueSlider.value = val;
  updateActivePresetValue('hue', val);
  try {
    await invoke('set_hue', { value: parseInt(val) });
  } catch (e) {
    console.error(e);
  }
}

// Sliders (User Interaction)
brightnessSlider.addEventListener('input', (e) => updateBrightness(e.target.value, true));
contrastSlider.addEventListener('input', (e) => updateContrast(e.target.value, true));
gammaSlider.addEventListener('input', (e) => updateGamma(e.target.value, true));
vibranceSlider.addEventListener('input', (e) => updateVibrance(e.target.value, true));
hueSlider.addEventListener('input', (e) => updateHue(e.target.value, true));

// Preset Management
let presets = JSON.parse(localStorage.getItem('irodori-presets')) || [
  { id: 'p1', name: '標準', brightness: 50, contrast: 50, gamma: 1.0, vibrance: 50, hue: 0, shortcut: null },
  { id: 'p2', name: 'ゲーム', brightness: 80, contrast: 70, gamma: 1.2, vibrance: 80, hue: 0, shortcut: 'G' },
  { id: 'p3', name: '映画', brightness: 40, contrast: 60, gamma: 0.8, vibrance: 60, hue: 0, shortcut: 'M' }
];

let activePresetId = localStorage.getItem('irodori-active-preset') || 'p1';
let recordingShortcutId = null;

function savePresets() {
  localStorage.setItem('irodori-presets', JSON.stringify(presets));
}

function renderPresets() {
  presetsList.innerHTML = '';
  presets.forEach(p => {
    const card = document.createElement('div');
    card.className = `preset-hero ${p.id === activePresetId ? 'active' : ''}`;
    card.dataset.id = p.id;
    
    const formatShortcut = (s) => {
      if (!s) return 'ショートカットキーを設定';
      return s.replace('Control', 'Ctrl').replace('Meta', 'Win').split('+').join(' + ');
    };

    const shortcutText = recordingShortcutId === p.id ? '入力待ち...' : (p.shortcut ? `[ ${formatShortcut(p.shortcut)} ]` : 'ショートカットキーを設定');

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
      </div>
    `;

    // Apply preset on click
    card.addEventListener('click', (e) => {
      if (e.target.closest('button') || e.target.closest('[contenteditable="true"]') || e.target.closest('input')) return;
      applyPreset(p.id);
    });

    // Shortcut recording logic
    const shortcutBtn = card.querySelector('.shortcut-btn');
    shortcutBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      recordingShortcutId = (recordingShortcutId === p.id) ? null : p.id;
      renderPresets();
    });

    // Rename logic
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
          nameEl.removeEventListener('blur', save);
        };
        nameEl.addEventListener('blur', save);
        nameEl.addEventListener('keydown', (ke) => {
          if (ke.key === 'Enter') { ke.preventDefault(); save(); }
        });
      }
    });

    // Delete logic
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
      card.appendChild(advancedSection);
      advancedSection.classList.remove('collapsed');
    }
  });
}

// Keyboard Listener
window.addEventListener('keydown', (e) => {
  // If recording a shortcut
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

      // Identify modifier keys
      const modifiers = [];
      if (e.ctrlKey) modifiers.push('Control');
      if (e.altKey) modifiers.push('Alt');
      if (e.shiftKey) modifiers.push('Shift');
      if (e.metaKey) modifiers.push('Meta');

      // If it's just a modifier key, don't finalize yet
      if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) {
        // Optional: show current held modifiers in UI
        return;
      }

      // Finalize shortcut
      const mainKey = e.key.toUpperCase();
      p.shortcut = modifiers.length > 0 ? `${modifiers.join('+')}+${mainKey}` : mainKey;
      
      savePresets();
      recordingShortcutId = null;
      renderPresets();
    }
    return;
  }

  // If typing in an editable element, don't trigger shortcuts
  if (document.activeElement.contentEditable === "true" || document.activeElement.tagName === "INPUT") return;

  // Match shortcut
  const currentKey = e.key.toUpperCase();
  const matched = presets.find(p => {
    if (!p.shortcut) return false;
    const parts = p.shortcut.split('+');
    const mainKey = parts.pop();
    const needsCtrl = parts.includes('Control');
    const needsAlt = parts.includes('Alt');
    const needsShift = parts.includes('Shift');
    const needsMeta = parts.includes('Meta');

    return currentKey === mainKey &&
           e.ctrlKey === needsCtrl &&
           e.altKey === needsAlt &&
           e.shiftKey === needsShift &&
           e.metaKey === needsMeta;
  });

  if (matched) {
    e.preventDefault();
    applyPreset(matched.id);
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
  updateHue(p.hue || 0);

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

renderPresets();
applyPreset(activePresetId);

listen('set-preset', (event) => {
  const p = presets.find(item => item.name === event.payload);
  if (p) applyPreset(p.id);
});

})();
