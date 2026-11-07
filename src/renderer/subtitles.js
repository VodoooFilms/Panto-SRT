'use strict';

// Subtítulos tab — Panto SRT's transcription, editing and export workflow.
(function () {
  const TEXT = {
    es: {
      dropTitle: 'Audio a subtítulos',
      dropHint: 'arrastra un archivo o haz clic para abrir',
      generate: 'Generar subtítulos',
      exportSrt: 'Exportar SRT',
      exportXml: 'Exportar FCPXML',
      cancel: 'Cancelar',
      downloadingModel: 'Descargando modelo…',
      modelReady: 'Modelo listo',
      transcribing: 'Transcribiendo…',
      cancelling: 'Cancelando…',
      editing: 'clips',
      regroup: 'Reagrupar',
      ready: 'Listo',
      exportReady: 'Exportado',
      exportDetails: () => 'Guardado junto al original',
      details: (count, duration) => `${count} clips · ${duration}`,
      selectFileFirst: 'Selecciona un audio o video primero.',
      models: 'Modelos',
      modelsSubtitle: 'Elige el motor de transcripción o libera espacio',
      modelsFoot: 'Toca un modelo para usarlo. Al borrar el activo se usará el otro si está descargado.',
      modelActive: 'Activo',
      modelReady: 'Listo',
      modelMissing: 'Sin descargar',
      modelDownloading: 'descargando…',
      modelOnDevice: 'en este equipo',
      useModel: 'Usar este modelo',
      downloadModel: 'Descargar modelo',
      deleteModel: 'Borrar modelo',
      deleteConfirm: (name) => `¿Borrar ${name}? Podrás descargarlo de nuevo cuando quieras.`,
      stopDownload: 'Detener descarga',
      unsupported: 'Formato no compatible.',
      removeFile: 'Quitar archivo',
      showFile: 'Mostrar archivo',
      close: 'Cerrar'
    },
    en: {
      dropTitle: 'Audio to subtitles',
      dropHint: 'drop a file or click to open',
      generate: 'Generate subtitles',
      exportSrt: 'Export SRT',
      exportXml: 'Export FCPXML',
      cancel: 'Cancel',
      downloadingModel: 'Downloading model…',
      modelReady: 'Model ready',
      transcribing: 'Transcribing…',
      cancelling: 'Cancelling…',
      editing: 'clips',
      regroup: 'Regroup',
      ready: 'Done',
      exportReady: 'Exported',
      exportDetails: () => 'Saved next to the original',
      details: (count, duration) => `${count} clips · ${duration}`,
      selectFileFirst: 'Select an audio or video file first.',
      models: 'Models',
      modelsSubtitle: 'Choose the transcription engine or free up space',
      modelsFoot: 'Tap a model to use it. Deleting the active one switches to the other if downloaded.',
      modelActive: 'Active',
      modelReady: 'Ready',
      modelMissing: 'Not downloaded',
      modelDownloading: 'downloading…',
      modelOnDevice: 'on this device',
      useModel: 'Use this model',
      downloadModel: 'Download model',
      deleteModel: 'Delete model',
      deleteConfirm: (name) => `Delete ${name}? You can download it again anytime.`,
      stopDownload: 'Stop download',
      unsupported: 'Format not supported.',
      removeFile: 'Remove file',
      showFile: 'Show file',
      close: 'Close'
    }
  };

  const SUBTITLE_DEFAULTS = { maxChars: 42, maxDuration: 3.5, minGap: 0.2, lang: 'auto', fps: 25 };
  const $ = (id) => document.getElementById(id);
  const tab = $('tab-subtitles');

  const state = {
    appLang: 'es',
    file: null,
    words: null,
    clips: [],
    transcribing: false,
    lastExport: null,
    models: [],
    downloading: {}
  };

  function u(key, ...args) {
    const value = TEXT[state.appLang][key];
    return typeof value === 'function' ? value(...args) : value;
  }

  function formatBytes(bytes) {
    const value = Number(bytes) || 0;
    if (value < 1024) return `${value} B`;
    const units = ['KB', 'MB', 'GB'];
    let index = -1;
    let size = value;
    do {
      size /= 1024;
      index += 1;
    } while (size >= 1024 && index < units.length - 1);
    return `${size.toFixed(size >= 100 ? 0 : size >= 10 ? 1 : 2)} ${units[index]}`;
  }

  function formatDuration(totalSeconds) {
    const total = Math.round(Number(totalSeconds) || 0);
    if (!total) return '';
    const minutes = Math.floor(total / 60);
    const secs = total % 60;
    const hours = Math.floor(minutes / 60);
    if (hours) return `${hours}:${String(minutes % 60).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
    return `${minutes}:${String(secs).padStart(2, '0')}`;
  }

  function secondsToStamp(seconds, includeMillis = true) {
    const total = Math.max(0, Math.round(seconds * 1000));
    const hours = Math.floor(total / 3600000);
    const minutes = Math.floor((total % 3600000) / 60000);
    const secs = Math.floor((total % 60000) / 1000);
    const millis = total % 1000;
    const pad = (n, w = 2) => String(n).padStart(w, '0');
    const body = `${pad(hours)}:${pad(minutes)}:${pad(secs)}`;
    return includeMillis ? `${body},${pad(millis, 3)}` : body;
  }

  const MODEL_ICONS = {
    'whisper-turbo': '<svg viewBox="0 0 20 20"><path d="M3 10h2l2-5 3 10 3-8 2 3h2"/></svg>',
    'whisper-small': '<svg viewBox="0 0 20 20"><path d="M3 10h2l2-5 3 10 3-8 2 3h2"/></svg>',
    parakeet: '<svg viewBox="0 0 20 20"><path d="M4 13V7M7.3 16V4M10.7 13.5v-7M14 16V4M17 11V9"/></svg>'
  };
  const STOP_SVG = '<svg viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" rx="2"/></svg>';
  const CHECK_SVG = '<svg viewBox="0 0 20 20"><path d="m4 10.5 4 4L16 6"/></svg>';
  const TRASH_SVG = '<svg viewBox="0 0 20 20"><path d="M3 5h14M8 5V3.5A.5.5 0 0 1 8.5 3h3a.5.5 0 0 1 .5.5V5m3 0-.7 10a1.5 1.5 0 0 1-1.5 1.4H7.2A1.5 1.5 0 0 1 5.7 15L5 5m4 3.5v5M11 8.5v5"/></svg>';
  const DOWNLOAD_SVG = '<svg viewBox="0 0 20 20"><path d="M10 3v10m0 0 4-4m-4 4L6 9M3.5 16.5h13"/></svg>';

  function renderModels() {
    const list = $('stModelsList');
    if (!list) return;
    list.textContent = '';
    for (const model of state.models) {
      const downloading = state.downloading[model.id];
      const isDownloading = downloading !== undefined;
      const row = document.createElement('div');
      row.className = `model-row${model.active ? '' : ' inactive'}`;

      const icon = document.createElement('div');
      icon.className = 'model-icon';
      icon.innerHTML = MODEL_ICONS[model.id] || MODEL_ICONS['whisper-turbo'];

      const copy = document.createElement('div');
      copy.className = 'model-copy';
      const name = document.createElement('strong');
      name.textContent = model.name;
      const detail = document.createElement('small');
      detail.textContent = isDownloading
        ? `${model.engineLabel} · ${u('modelDownloading')}`
        : `${model.engineLabel} · ES / EN · ${u('modelOnDevice')}`;
      copy.append(name, detail);

      const status = document.createElement('div');
      status.className = 'model-status';
      if (isDownloading) {
        const pct = document.createElement('span');
        pct.className = 'model-pct';
        pct.textContent = `${downloading}% · ${formatBytes((model.size * downloading) / 100)} / ${formatBytes(model.size)}`;
        const track = document.createElement('div');
        track.className = 'mini-track';
        const bar = document.createElement('div');
        bar.style.width = `${downloading}%`;
        track.append(bar);
        status.append(pct, track);
      } else {
        const badge = document.createElement('span');
        badge.className = `model-badge${model.active ? ' active' : ''}`;
        badge.textContent = model.active ? u('modelActive') : model.downloaded ? u('modelReady') : u('modelMissing');
        const size = document.createElement('span');
        size.className = 'model-size';
        size.textContent = formatBytes(model.size);
        status.append(badge, size);
      }

      const actions = document.createElement('div');
      actions.className = 'model-actions';
      const slot = document.createElement('button');
      if (!model.downloaded && !isDownloading) {
        slot.innerHTML = DOWNLOAD_SVG;
        slot.title = u('downloadModel');
        slot.setAttribute('aria-label', u('downloadModel'));
        slot.addEventListener('click', (event) => {
          event.stopPropagation();
          downloadModel(model);
        });
      } else {
        slot.className = model.active ? 'check-slot' : 'check-slot empty';
        slot.innerHTML = CHECK_SVG;
        slot.title = model.active ? u('modelActive') : u('useModel');
        slot.setAttribute('aria-label', slot.title);
        if (isDownloading) slot.disabled = true;
      }
      const trash = document.createElement('button');
      trash.className = 'danger';
      trash.innerHTML = TRASH_SVG;
      trash.title = u('deleteModel');
      trash.setAttribute('aria-label', u('deleteModel'));
      trash.disabled = !model.downloaded || isDownloading;
      trash.addEventListener('click', async (event) => {
        event.stopPropagation();
        if (!window.confirm(u('deleteConfirm', model.name))) return;
        try {
          state.models = await window.pantoSRT.deleteModel(model.id);
          renderModels();
        } catch (error) {
          showError((error && error.message) || 'Error');
        }
      });
      actions.append(slot, trash);

      if (isDownloading) {
        const stop = document.createElement('button');
        stop.className = 'stop-download';
        stop.innerHTML = STOP_SVG;
        stop.title = u('stopDownload');
        stop.setAttribute('aria-label', u('stopDownload'));
        stop.addEventListener('click', (event) => {
          event.stopPropagation();
          window.pantoSRT.cancelDownload();
        });
        row.append(icon, copy, stop, status, actions);
      } else {
        row.append(icon, copy, status, actions);
      }
      row.addEventListener('click', async () => {
        if (!model.downloaded || model.active || isDownloading) return;
        try {
          state.models = await window.pantoSRT.setActiveModel(model.id);
          renderModels();
        } catch (error) {
          showError((error && error.message) || 'Error');
        }
      });
      list.append(row);
    }
  }

  async function loadModels() {
    try {
      state.models = await window.pantoSRT.listModels();
      renderModels();
    } catch (_) {
      // Panel stays empty until the backend answers.
    }
  }

  async function downloadModel(model) {
    state.downloading[model.id] = 0;
    renderModels();
    try {
      state.models = await window.pantoSRT.downloadModel(model.id);
    } catch (error) {
      if (!error || error.message !== 'cancelled') showError((error && error.message) || 'Error');
    } finally {
      delete state.downloading[model.id];
      renderModels();
    }
  }

  function rowLabel(clip) {
    return `${secondsToStamp(clip.start)} → ${secondsToStamp(clip.end)}`;
  }

  function renderClipRows() {
    const list = $('stClipList');
    list.textContent = '';
    state.clips.forEach((clip, index) => {
      const row = document.createElement('div');
      row.className = 'clip-row';

      const indexCell = document.createElement('span');
      indexCell.className = 'clip-index';
      indexCell.textContent = String(clip.index);

      const rangeCell = document.createElement('span');
      rangeCell.className = 'clip-range';
      rangeCell.dataset.index = index;

      const textInput = document.createElement('input');
      textInput.className = 'clip-text';
      textInput.type = 'text';
      textInput.value = clip.text;
      textInput.spellcheck = false;
      textInput.addEventListener('input', () => { state.clips[index].text = textInput.value; });

      row.append(indexCell, rangeCell, textInput);
      list.append(row);
    });
    refreshRanges();
    $('stEditorCount').textContent = `${state.clips.length} ${u('editing')}`;
  }

  function refreshRanges() {
    state.clips.forEach((clip, index) => {
      const cell = document.querySelector(`#stClipList .clip-range[data-index="${index}"]`);
      if (cell) cell.textContent = rowLabel(clip);
    });
  }

  function showError(message) {
    const el = $('stErrorMessage');
    if (!message) {
      el.classList.add('hidden');
      return;
    }
    el.textContent = message;
    el.classList.remove('hidden');
  }

  function showResult(titleKey, details = '', revealPath = null) {
    $('stResultTitle').textContent = u(titleKey);
    $('stResultDetails').textContent = details;
    $('stResultReveal').classList.toggle('hidden', !revealPath);
    $('stResultPanel').classList.remove('hidden');
    if (revealPath) state.lastExport = { path: revealPath };
  }

  function hideResult() {
    $('stResultPanel').classList.add('hidden');
    state.lastExport = null;
  }

  function setBusy(busy) {
    state.transcribing = busy;
    $('stGenerateButton').disabled = busy || !state.file;
    $('stCancelButton').classList.toggle('hidden', !busy);
    $('stDropZone').style.pointerEvents = busy ? 'none' : '';
  }

  async function pickFromDialog() {
    try {
      const file = await window.pantoSRT.selectMedia();
      if (file) setFile(file);
    } catch (error) {
      showError((error && error.message) || 'Error');
    }
  }

  function setFile(file) {
    tab.classList.add('has-file');
    state.file = { ...file };
    state.words = null;
    state.clips = [];
    hideResult();
    showError('');
    $('stDropZone').classList.add('hidden');
    $('stFileCard').classList.remove('hidden');
    $('stFileName').textContent = file.name;
    const meta = [formatBytes(file.size), file.duration ? formatDuration(file.duration) : ''].filter(Boolean).join(' · ');
    $('stFileMeta').textContent = meta;
    $('stEditorPanel').classList.add('hidden');
    $('stGenerateButton').classList.remove('hidden');
    $('stExportSrtButton').classList.add('hidden');
    $('stExportXmlButton').classList.add('hidden');
    $('stGenerateButton').disabled = false;
  }

  function reset() {
    tab.classList.remove('has-file');
    state.file = null;
    state.words = null;
    state.clips = [];
    state.lastExport = null;
    showError('');
    hideResult();
    $('stFileCard').classList.add('hidden');
    $('stDropZone').classList.remove('hidden');
    $('stEditorPanel').classList.add('hidden');
    $('stProgressPanel').classList.add('hidden');
    $('stGenerateButton').disabled = true;
    $('stGenerateButton').classList.remove('hidden');
    $('stExportSrtButton').classList.add('hidden');
    $('stExportXmlButton').classList.add('hidden');
    $('stCancelButton').classList.add('hidden');
  }

  function setProgress(percent, label) {
    $('stProgressPercent').textContent = `${Math.round(Number(percent) || 0)}%`;
    $('stProgressBar').style.width = `${Math.min(100, Math.max(0, percent))}%`;
    if (label) $('stProgressLabel').textContent = label;
    $('stProgressTime').textContent = '';
  }

  async function generate() {
    if (!state.file) {
      showError(u('selectFileFirst'));
      return;
    }
    showError('');
    hideResult();
    setBusy(true);
    $('stEditorPanel').classList.add('hidden');
    $('stProgressPanel').classList.remove('hidden');
    setProgress(0, u('transcribing'));

    try {
      const result = await window.pantoSRT.transcribeMedia(state.file.path, {
        language: SUBTITLE_DEFAULTS.lang,
        mediaDuration: state.file.duration
      });
      state.words = result.words;
      state.clips = await window.pantoSRT.groupClips(result.words, SUBTITLE_DEFAULTS);
      $('stProgressPanel').classList.add('hidden');
      $('stEditorPanel').classList.remove('hidden');
      $('stExportSrtButton').classList.remove('hidden');
      $('stExportXmlButton').classList.remove('hidden');
      renderClipRows();
      showResult('ready', u('details', state.clips.length, formatDuration(result.duration || 0)));
    } catch (error) {
      $('stProgressPanel').classList.add('hidden');
      if (error && error.message !== 'cancelled') showError(error.message || 'Error');
    } finally {
      setBusy(false);
    }
  }

  async function regroup() {
    if (!state.words || !state.words.length) return;
    const previous = new Map(state.clips.map((clip) => [Number(clip.start).toFixed(3), clip.text]));
    const next = await window.pantoSRT.groupClips(state.words, SUBTITLE_DEFAULTS);
    next.forEach((clip) => {
      const edited = previous.get(Number(clip.start).toFixed(3));
      if (edited !== undefined) clip.text = edited;
    });
    state.clips = next;
    renderClipRows();
    showResult('ready', u('details', state.clips.length, formatDuration(state.file.duration || 0)));
  }

  async function exportSubtitle(format) {
    if (!state.clips.length) return;
    showError('');
    try {
      const result = await window.pantoSRT.exportSubtitles(state.file.path, format, state.clips, {
        timecodeBase: 0,
        fps: SUBTITLE_DEFAULTS.fps
      });
      showResult('exportReady', u('exportDetails', result.path), result.outputPath);
    } catch (error) {
      showError((error && error.message) || 'Error');
    }
  }

  async function handleDrop(paths) {
    const filePath = paths && paths[0];
    if (state.file || !filePath) return;
    try {
      const details = await window.pantoSRT.inspectMedia(filePath);
      setFile(details);
    } catch (_) {
      showError(u('unsupported'));
    }
  }

  function setModelsView(open) {
    $('stModelsPanel').classList.toggle('hidden', !open);
    tab.classList.toggle('models-open', open);
    if (open) loadModels();
  }

  function applyLanguage(language) {
    state.appLang = TEXT[language] ? language : 'es';
    $('stDropTitle').textContent = u('dropTitle');
    $('stDropHint').textContent = u('dropHint');
    $('stGenerateButton').textContent = u('generate');
    $('stExportSrtButton').textContent = u('exportSrt');
    $('stExportXmlButton').textContent = u('exportXml');
    $('stCancelButton').textContent = u('cancel');
    $('stRegroupButton').textContent = u('regroup');
    $('stModelsToggle').textContent = u('models');
    $('stModelsTitle').textContent = u('models');
    $('stModelsSubtitle').textContent = u('modelsSubtitle');
    $('stModelsFoot').textContent = u('modelsFoot');
    $('stDropZone').setAttribute('aria-label', u('dropTitle'));
    $('stRemoveButton').setAttribute('aria-label', u('removeFile'));
    $('stRemoveButton').title = u('removeFile');
    $('stResultReveal').setAttribute('aria-label', u('showFile'));
    $('stResultReveal').title = u('showFile');
    $('stModelsClose').setAttribute('aria-label', u('close'));
    $('stModelsClose').title = u('close');
    $('stModelsToggle').setAttribute('aria-label', u('models'));
    $('stModelsToggle').title = u('models');
    renderModels();
  }

  function wire() {
    $('stModelsToggle').addEventListener('click', () => {
      setModelsView($('stModelsPanel').classList.contains('hidden'));
    });
    $('stModelsClose').addEventListener('click', () => setModelsView(false));

    $('stDropZone').addEventListener('click', pickFromDialog);
    $('stDropZone').addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        pickFromDialog();
      }
    });
    $('stRemoveButton').addEventListener('click', reset);
    $('stGenerateButton').addEventListener('click', generate);
    $('stCancelButton').addEventListener('click', () => window.pantoSRT.cancelTranscription());
    $('stRegroupButton').addEventListener('click', regroup);
    $('stExportSrtButton').addEventListener('click', () => exportSubtitle('srt'));
    $('stExportXmlButton').addEventListener('click', () => exportSubtitle('fcpxml'));
    $('stResultReveal').addEventListener('click', () => {
      if (state.lastExport) window.pantoSRT.showInFolder(state.lastExport.path);
    });

    window.pantoSRT.onModelDownloadProgress((payload) => {
      if (state.downloading[payload.id] !== undefined) {
        state.downloading[payload.id] = payload.percent;
        renderModels();
      }
    });

    window.pantoSRT.onTranscriptionStatus((payload) => {
      if (payload.status === 'model-download') setProgress(0, u('downloadingModel'));
      else if (payload.status === 'model-ready') setProgress(0, u('modelReady'));
      else if (payload.status === 'starting') setProgress(0, u('transcribing'));
      else if (payload.status === 'cancelling') setProgress(0, u('cancelling'));
    });
    window.pantoSRT.onTranscriptionProgress((payload) => {
      if (payload.phase === 'model') setProgress(payload.percent, u('downloadingModel'));
      else setProgress(payload.percent);
    });
  }

  function init(language) {
    $('stResultReveal').textContent = window.pantoSRT.platform === 'darwin' ? '⌘' : '◎';
    wire();
    applyLanguage(language);
    loadModels();
  }

  window.SubtitlesTab = {
    init,
    setLanguage: applyLanguage,
    handleDrop
  };
})();
