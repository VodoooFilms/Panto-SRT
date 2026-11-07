'use strict';

// Upscale tab — Panto Upscaler's Lanczos workflow.
(function () {
  const COPY = {
    es: {
      selectAria: 'Seleccionar un vídeo',
      dropTitle: 'Arrastra un vídeo',
      dropHint: 'o haz clic para abrir',
      formats: 'MP4 · MOV · MKV · WEBM · AVI',
      input: 'Entrada',
      output: 'Salida',
      noFile: 'Sin archivo',
      targetsAria: 'Resolución de salida',
      modesAria: 'Ajuste de imagen',
      modes: { pad: 'Barras negras', crop: 'Recortar', stretch: 'Estirar' },
      scaleTo: (label) => `Escalar a ${label}`,
      ready: 'Lanczos',
      readyDetail: 'Calidad máxima · CRF 14',
      scanning: 'Analizando vídeo…',
      scanningDetail: 'Leyendo pistas y metadatos',
      scaling: 'Escalando…',
      scalingDetail: 'Lanczos · precisión de color completa',
      done: 'Listo',
      error: 'Error',
      bars: 'barras',
      crop: 'recorte',
      noBars: 'sin barras',
      noCrop: 'sin recorte',
      larger: 'más grande',
      smaller: 'más pequeño',
      similar: 'similar',
      estimate: 'aprox.',
      cancel: 'Cancelar',
      another: 'Escalar otro',
      reveal: 'Mostrar en Finder',
      remove: 'Quitar vídeo',
      folder: 'Elegir carpeta de salida',
      engineTitle: 'Escalador Lanczos de FFmpeg',
      ffmpegMissing: 'No se encontró FFmpeg. Instálalo con "brew install ffmpeg".',
      unsupported: 'Ese archivo no es un vídeo compatible.',
      busy: 'Ya hay una conversión en curso.',
      timeout: 'FFmpeg tardó demasiado. Prueba con un archivo más pequeño.',
      failed: 'No se pudo completar el escalado.',
      quality: 'H.264 · CRF 14',
      footer: 'Procesado en tu equipo · sin subir nada'
    },
    en: {
      selectAria: 'Select a video',
      dropTitle: 'Drop a video',
      dropHint: 'or click to browse',
      formats: 'MP4 · MOV · MKV · WEBM · AVI',
      input: 'Input',
      output: 'Output',
      noFile: 'No file',
      targetsAria: 'Output resolution',
      modesAria: 'Image fit',
      modes: { pad: 'Black bars', crop: 'Crop', stretch: 'Stretch' },
      scaleTo: (label) => `Upscale to ${label}`,
      ready: 'Lanczos',
      readyDetail: 'Maximum quality · CRF 14',
      scanning: 'Analyzing video…',
      scanningDetail: 'Reading tracks and metadata',
      scaling: 'Upscaling…',
      scalingDetail: 'Lanczos · full chroma precision',
      done: 'Done',
      error: 'Error',
      bars: 'bars',
      crop: 'crop',
      noBars: 'no bars',
      noCrop: 'no crop',
      larger: 'larger',
      smaller: 'smaller',
      similar: 'similar',
      estimate: 'approx.',
      cancel: 'Cancel',
      another: 'Upscale another',
      reveal: 'Show in Finder',
      remove: 'Remove video',
      folder: 'Choose output folder',
      engineTitle: 'FFmpeg Lanczos scaler',
      ffmpegMissing: 'FFmpeg was not found. Install it with "brew install ffmpeg".',
      unsupported: 'That file is not a supported video.',
      busy: 'A conversion is already in progress.',
      timeout: 'FFmpeg took too long. Try a smaller file.',
      failed: 'The upscale could not be completed.',
      quality: 'H.264 · CRF 14',
      footer: 'Processed on your device · nothing uploaded'
    }
  };

  const TARGETS = {
    720: { width: 1280, height: 720, label: '720p' },
    1080: { width: 1920, height: 1080, label: '1080p' },
    1440: { width: 2560, height: 1440, label: '1440p' },
    2160: { width: 3840, height: 2160, label: '4K' }
  };

  const $ = (id) => document.getElementById(id);
  const tab = $('tab-upscale');
  const inputCard = $('upInputCard');
  const inputThumb = $('upInputThumb');
  const inputEmpty = $('upInputEmpty');
  const inputBadge = $('upInputBadge');
  const removeButton = $('upRemoveButton');
  const outputThumb = $('upOutputThumb');
  const outputEmpty = $('upOutputEmpty');
  const outputBadge = $('upOutputBadge');
  const folderButton = $('upFolderButton');
  const targetGroup = $('upTargetGroup');
  const modeGroup = $('upModeGroup');
  const convertButton = $('upConvertButton');
  const cancelButton = $('upCancelButton');
  const anotherButton = $('upAnotherButton');
  const revealButton = $('upRevealButton');
  const statusPanel = $('upStatusPanel');
  const statusIcon = $('upStatusIcon');
  const statusTitle = $('upStatusTitle');
  const statusDetail = $('upStatusDetail');
  const statusRight = $('upStatusRight');
  const progressTrack = $('upProgressTrack');
  const progressBar = $('upProgressBar');

  const state = {
    language: 'es',
    file: null,
    plan: null,
    target: '1080',
    mode: 'crop',
    outputDirectory: null,
    converting: false,
    loading: false,
    cancelRequested: false,
    result: null,
    planRequest: 0,
    ffmpegAvailable: true
  };

  const copy = () => COPY[state.language];
  const t = (key) => copy()[key];

  function formatBytes(bytes) {
    if (!Number.isFinite(bytes) || bytes <= 0) return '0 MB';
    if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
    const megabytes = bytes / 1024 ** 2;
    return `${megabytes < 1 ? megabytes.toFixed(2) : megabytes.toFixed(1)} MB`;
  }

  function formatDuration(seconds) {
    if (!Number.isFinite(seconds) || seconds <= 0) return '';
    const total = Math.round(seconds);
    const hours = Math.floor(total / 3600);
    const minutes = Math.floor((total % 3600) / 60);
    const remaining = total % 60;
    if (hours) return `${hours}:${String(minutes).padStart(2, '0')}:${String(remaining).padStart(2, '0')}`;
    return `${minutes}:${String(remaining).padStart(2, '0')}`;
  }

  function formatFps(fps) {
    if (!Number.isFinite(fps) || fps <= 0) return '';
    return Number.isInteger(fps) ? `${fps}` : fps.toFixed(2).replace(/0+$/, '').replace(/\.$/, '');
  }

  function fileStem(name) {
    return name.replace(/\.[^.]+$/, '');
  }

  function planBars(plan) {
    if (!plan) return '';
    if (plan.mode === 'crop') {
      if (plan.cropLeft && plan.cropTop) return `${t('crop')} ${plan.cropLeft}×${plan.cropTop} px`;
      if (plan.cropLeft) return `${t('crop')} ${plan.cropLeft} px`;
      if (plan.cropTop) return `${t('crop')} ${plan.cropTop} px`;
      return t('noCrop');
    }
    if (plan.mode === 'stretch') return t('noBars');
    if (plan.barLeft && plan.barTop) return `${t('bars')} ${plan.barLeft}×${plan.barTop} px`;
    if (plan.barLeft) return `${t('bars')} ${plan.barLeft} px`;
    if (plan.barTop) return `${t('bars')} ${plan.barTop} px`;
    return t('noBars');
  }

  function sizeDelta(result) {
    if (!result || !result.inputBytes || !result.outputBytes) return '';
    const ratio = result.outputBytes / result.inputBytes;
    if (ratio >= 1.05) return `+${Math.round((ratio - 1) * 100)}% ${t('larger')}`;
    if (ratio <= 0.95) return `−${Math.round((1 - ratio) * 100)}% ${t('smaller')}`;
    return t('similar');
  }

  function targetInfo() {
    return TARGETS[state.target] || TARGETS['1080'];
  }

  function setStatusIdle() {
    statusPanel.className = 'status idle';
    statusIcon.textContent = '';
    statusTitle.textContent = t('ready');
    statusDetail.textContent = t('readyDetail');
    statusRight.textContent = state.plan ? `≈ ${formatBytes(state.plan.estimatedBytes)}` : '';
    progressTrack.classList.add('hidden');
  }

  function setStatusLoading() {
    statusPanel.className = 'status processing';
    statusIcon.textContent = '';
    statusTitle.textContent = t('scanning');
    statusDetail.textContent = t('scanningDetail');
    statusRight.textContent = '';
    progressTrack.classList.add('hidden');
  }

  function setStatusProcessing(percent) {
    statusPanel.className = 'status processing';
    statusIcon.textContent = '';
    statusTitle.textContent = t('scaling');
    statusDetail.textContent = state.plan ? `${state.plan.label} · ${t('scalingDetail')}` : t('scalingDetail');
    statusRight.textContent = `${Math.round(percent)}%`;
    progressTrack.classList.remove('hidden');
    progressBar.style.width = `${Math.max(0, Math.min(100, percent))}%`;
  }

  function setStatusSuccess(result) {
    statusPanel.className = 'status success';
    statusIcon.textContent = '✓';
    statusTitle.textContent = t('done');
    statusDetail.textContent = `${result.plan.targetWidth} × ${result.plan.targetHeight} · ${formatBytes(result.outputBytes)}`;
    statusRight.textContent = sizeDelta(result);
    progressTrack.classList.add('hidden');
  }

  function setStatusError(message) {
    statusPanel.className = 'status error';
    statusIcon.textContent = '!';
    statusTitle.textContent = t('error');
    statusDetail.textContent = message;
    statusRight.textContent = '';
    progressTrack.classList.add('hidden');
  }

  function renderInput() {
    const file = state.file;
    const hasThumb = Boolean(file && file.thumbnail);

    inputEmpty.classList.toggle('hidden', Boolean(file));
    inputThumb.classList.toggle('hidden', !hasThumb);
    inputBadge.classList.toggle('hidden', !(hasThumb && file && file.width));
    removeButton.classList.toggle('hidden', !file);

    inputCard.setAttribute('aria-label', file ? `${file.name}, ${file.width} × ${file.height}` : t('selectAria'));

    if (file) {
      if (hasThumb) inputThumb.src = file.thumbnail;
      else inputThumb.removeAttribute('src');

      $('upInputName').textContent = file.name;
      $('upInputDimensions').textContent = file.width ? `${file.width} × ${file.height}` : '—';
      $('upInputSize').textContent = formatBytes(file.size);
      $('upInputDetail').textContent = [
        file.codec ? file.codec.toUpperCase() : '',
        file.fps ? `${formatFps(file.fps)} fps` : '',
        formatDuration(file.duration)
      ].filter(Boolean).join(' · ');
      inputBadge.textContent = `${file.width} × ${file.height}`;
    } else {
      inputThumb.removeAttribute('src');
      $('upInputName').textContent = t('noFile');
      $('upInputDimensions').textContent = '—';
      $('upInputSize').textContent = '—';
      $('upInputDetail').textContent = t('formats');
      inputBadge.textContent = '';
    }
  }

  function renderOutput() {
    const plan = state.plan;
    const result = state.result;

    if (result) {
      outputEmpty.classList.add('hidden');
      outputThumb.classList.remove('hidden');
      outputBadge.classList.remove('hidden');
      outputThumb.src = result.thumbnail || (state.file && state.file.thumbnail) || '';
      outputBadge.textContent = `${result.plan.targetWidth} × ${result.plan.targetHeight}`;
      $('upOutputName').textContent = result.outputName;
      $('upOutputDimensions').textContent = `${result.plan.targetWidth} × ${result.plan.targetHeight}`;
      $('upOutputSize').textContent = formatBytes(result.outputBytes);
      $('upOutputDetail').textContent = `H.264 · ${planBars(result.plan)}`;
      return;
    }

    if (state.file && plan) {
      outputEmpty.classList.add('hidden');
      outputBadge.classList.add('hidden');
      outputThumb.classList.toggle('hidden', !state.file.thumbnail);
      if (state.file.thumbnail) outputThumb.src = state.file.thumbnail;
      else outputThumb.removeAttribute('src');

      $('upOutputName').textContent = `${fileStem(state.file.name)}_${plan.label.toLowerCase()}.mp4`;
      $('upOutputDimensions').textContent = `${plan.targetWidth} × ${plan.targetHeight}`;
      $('upOutputSize').textContent = plan.estimatedBytes ? `≈ ${formatBytes(plan.estimatedBytes)}` : '—';
      $('upOutputDetail').textContent = `Lanczos · ${planBars(plan)}`;
      return;
    }

    const target = targetInfo();
    outputThumb.classList.add('hidden');
    outputThumb.removeAttribute('src');
    outputBadge.classList.add('hidden');
    outputEmpty.classList.remove('hidden');
    $('upOutputTargetChip').textContent = target.label;
    $('upOutputTargetDims').textContent = `${target.width} × ${target.height}`;
    $('upOutputName').textContent = '—';
    $('upOutputDimensions').textContent = '—';
    $('upOutputSize').textContent = '—';
    $('upOutputDetail').textContent = `Lanczos · ${t('quality')}`;
  }

  function renderActions() {
    const showConvert = !state.converting && !state.result;
    const showCancel = state.converting;
    const showResult = !state.converting && Boolean(state.result);

    convertButton.classList.toggle('hidden', !showConvert);
    cancelButton.classList.toggle('hidden', !showCancel);
    anotherButton.classList.toggle('hidden', !showResult);
    revealButton.classList.toggle('hidden', !showResult);

    convertButton.disabled = !state.file || state.loading || !state.ffmpegAvailable;
    convertButton.textContent = t('scaleTo')((state.plan && state.plan.label) || targetInfo().label);
    cancelButton.textContent = t('cancel');
    anotherButton.textContent = t('another');
    revealButton.textContent = t('reveal');
  }

  function renderTargetSelection() {
    for (const button of targetGroup.querySelectorAll('[data-target]')) {
      const active = button.dataset.target === state.target;
      button.classList.toggle('active', active);
      button.setAttribute('aria-pressed', active ? 'true' : 'false');
    }
    for (const button of modeGroup.querySelectorAll('[data-mode]')) {
      const active = button.dataset.mode === state.mode;
      button.classList.toggle('active', active);
      button.setAttribute('aria-pressed', active ? 'true' : 'false');
    }
  }

  function renderFolderButton() {
    const custom = Boolean(state.outputDirectory);
    folderButton.classList.toggle('custom', custom);
    folderButton.title = custom ? state.outputDirectory : t('folder');
    folderButton.setAttribute('aria-label', t('folder'));
  }

  async function refreshPlan() {
    if (!state.file) {
      state.plan = null;
      renderOutput();
      if (!state.converting && !state.result) setStatusIdle();
      renderActions();
      return;
    }

    const request = ++state.planRequest;
    try {
      const plan = await window.pantoUpscale.computePlan({
        width: state.file.width,
        height: state.file.height,
        duration: state.file.duration,
        audioKbps: state.file.audioKbps || 0,
        target: state.target,
        mode: state.mode
      });
      if (request !== state.planRequest) return;
      state.plan = plan;
      renderOutput();
      if (!state.converting && !state.result) setStatusIdle();
      renderActions();
    } catch (error) {
      if (request !== state.planRequest) return;
      handleError(error);
    }
  }

  function setFile(file) {
    state.file = file;
    state.result = null;
    state.loading = false;
    state.cancelRequested = false;
    tab.classList.remove('is-processing', 'is-complete');
    renderInput();
    renderOutput();
    renderActions();
    refreshPlan();
  }

  function resetFile() {
    if (state.converting) return;
    state.file = null;
    state.plan = null;
    state.result = null;
    state.loading = false;
    state.planRequest += 1;
    tab.classList.remove('is-processing', 'is-complete');
    renderInput();
    renderOutput();
    renderActions();
    setStatusIdle();
  }

  async function chooseFile() {
    if (state.converting || state.loading) return;
    state.loading = true;
    renderActions();
    setStatusLoading();
    try {
      const file = await window.pantoUpscale.selectVideo();
      if (!file) {
        state.loading = false;
        renderActions();
        if (state.result) setStatusSuccess(state.result);
        else setStatusIdle();
        return;
      }
      setFile(file);
    } catch (error) {
      state.loading = false;
      handleError(error);
    }
  }

  async function handleDrop(paths) {
    const path = paths && paths[0];
    if (!path || state.converting) return;
    state.loading = true;
    renderActions();
    setStatusLoading();
    try {
      const file = await window.pantoUpscale.inspectVideo(path);
      setFile(file);
    } catch (error) {
      state.loading = false;
      handleError(error);
    }
  }

  async function chooseOutputFolder() {
    if (state.converting) return;
    try {
      const folder = await window.pantoUpscale.chooseFolder();
      if (!folder) return;
      state.outputDirectory = folder;
      renderFolderButton();
    } catch (error) {
      handleError(error);
    }
  }

  function handleError(error) {
    state.loading = false;
    state.converting = false;
    tab.classList.remove('is-processing');
    const raw = typeof error === 'string' ? error : (error && error.message) || '';
    let message;
    if (raw.startsWith('FFMPEG_MISSING')) message = t('ffmpegMissing');
    else if (raw.startsWith('UNSUPPORTED')) message = t('unsupported');
    else if (raw.startsWith('BUSY')) message = t('busy');
    else if (raw.startsWith('FFMPEG_TIMEOUT')) message = t('timeout');
    else if (raw.startsWith('FFMPEG_FAILED:')) message = raw.slice('FFMPEG_FAILED:'.length).trim() || t('failed');
    else message = raw || t('failed');
    setStatusError(message);
    renderActions();
  }

  async function convert() {
    if (!state.file || state.converting || state.loading) return;

    state.converting = true;
    state.cancelRequested = false;
    tab.classList.add('is-processing');
    tab.classList.remove('is-complete');
    renderActions();
    setStatusProcessing(0);

    try {
      const result = await window.pantoUpscale.start({
        inputPath: state.file.path,
        targetId: state.target,
        mode: state.mode,
        outputDirectory: state.outputDirectory
      });

      state.converting = false;
      state.result = result;
      tab.classList.remove('is-processing');
      tab.classList.add('is-complete');
      renderOutput();
      setStatusSuccess(result);
      renderActions();
      playChime();
    } catch (error) {
      const raw = typeof error === 'string' ? error : (error && error.message) || '';
      state.converting = false;
      tab.classList.remove('is-processing');
      if (state.cancelRequested || raw.startsWith('CANCELLED')) {
        state.result = null;
        renderOutput();
        setStatusIdle();
        renderActions();
      } else {
        handleError(error);
      }
    }
  }

  async function cancel() {
    if (!state.converting) return;
    state.cancelRequested = true;
    cancelButton.disabled = true;
    try {
      await window.pantoUpscale.cancel();
    } catch (_) {
      // The running command reports the final state.
    } finally {
      cancelButton.disabled = false;
    }
  }

  function playChime() {
    const AudioContextClass = window.AudioContext || window.webkitAudioContext;
    if (!AudioContextClass) return;
    try {
      const context = new AudioContextClass();
      const now = context.currentTime;
      [660, 880].forEach((frequency, index) => {
        const oscillator = context.createOscillator();
        const gain = context.createGain();
        const start = now + index * 0.11;
        oscillator.type = 'sine';
        oscillator.frequency.value = frequency;
        gain.gain.setValueAtTime(0, start);
        gain.gain.linearRampToValueAtTime(0.055, start + 0.02);
        gain.gain.exponentialRampToValueAtTime(0.0001, start + 0.32);
        oscillator.connect(gain).connect(context.destination);
        oscillator.start(start);
        oscillator.stop(start + 0.36);
      });
      setTimeout(() => context.close(), 900);
    } catch (_) {
      // Audio is a nice-to-have.
    }
  }

  function selectTarget(target) {
    if (state.converting || !TARGETS[target] || target === state.target) return;
    state.target = target;
    state.result = null;
    state.planRequest += 1;
    tab.classList.remove('is-complete');
    renderTargetSelection();
    renderOutput();
    renderActions();
    refreshPlan();
  }

  function selectMode(mode) {
    if (state.converting || !['pad', 'crop', 'stretch'].includes(mode) || mode === state.mode) return;
    state.mode = mode;
    state.result = null;
    state.planRequest += 1;
    tab.classList.remove('is-complete');
    renderTargetSelection();
    renderOutput();
    renderActions();
    refreshPlan();
  }

  function applyLanguage(language) {
    if (!COPY[language]) return;
    state.language = language;
    $('upEngineBadge').title = t('engineTitle');
    $('upFooterNote').textContent = t('footer');
    $('upInputKicker').textContent = t('input');
    $('upOutputKicker').textContent = t('output');
    $('upDropTitle').textContent = t('dropTitle');
    $('upDropHint').textContent = t('dropHint');
    targetGroup.setAttribute('aria-label', t('targetsAria'));
    modeGroup.setAttribute('aria-label', t('modesAria'));
    for (const button of modeGroup.querySelectorAll('[data-mode]')) {
      button.textContent = t('modes')[button.dataset.mode];
    }
    removeButton.setAttribute('aria-label', t('remove'));
    removeButton.title = t('remove');
    renderFolderButton();
    renderInput();
    renderOutput();
    renderTargetSelection();
    renderActions();
    if (state.converting) setStatusProcessing(Number(statusRight.textContent.replace('%', '')) || 0);
    else if (state.result) setStatusSuccess(state.result);
    else if (!state.ffmpegAvailable) setStatusError(t('ffmpegMissing'));
    else setStatusIdle();
  }

  function wire() {
    inputCard.addEventListener('click', (event) => {
      if (event.target.closest('#upRemoveButton, #upFolderButton')) return;
      chooseFile();
    });
    inputCard.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        chooseFile();
      }
    });
    removeButton.addEventListener('click', (event) => {
      event.stopPropagation();
      resetFile();
    });
    folderButton.addEventListener('click', (event) => {
      event.stopPropagation();
      chooseOutputFolder();
    });
    targetGroup.addEventListener('click', (event) => {
      const button = event.target.closest('[data-target]');
      if (button) selectTarget(button.dataset.target);
    });
    modeGroup.addEventListener('click', (event) => {
      const button = event.target.closest('[data-mode]');
      if (button) selectMode(button.dataset.mode);
    });
    convertButton.addEventListener('click', convert);
    cancelButton.addEventListener('click', cancel);
    anotherButton.addEventListener('click', resetFile);
    revealButton.addEventListener('click', () => {
      if (state.result) window.pantoUpscale.showInFolder(state.result.outputPath);
    });
    document.addEventListener('keydown', (event) => {
      if (event.key === 'Escape' && state.converting) {
        event.preventDefault();
        cancel();
      }
    });
    window.pantoUpscale.onProgress(({ percent }) => {
      if (!state.converting) return;
      const value = Number(percent);
      if (Number.isFinite(value)) setStatusProcessing(value);
    });
  }

  function init(language) {
    state.language = COPY[language] ? language : 'es';
    wire();
    applyLanguage(state.language);
    renderTargetSelection();
    renderFolderButton();
    renderActions();
    setStatusIdle();
    window.pantoUpscale.status()
      .then((status) => {
        if (!status || !status.available) {
          state.ffmpegAvailable = false;
          setStatusError(t('ffmpegMissing'));
          renderActions();
        }
      })
      .catch(() => {});
  }

  window.UpscaleTab = {
    init,
    setLanguage: applyLanguage,
    handleDrop
  };
})();
