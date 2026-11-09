'use strict';

// Convertir tab — Pantoraya's MP4 / MP3 / JPG workflows.
(function () {
  const COPY = {
    es: {
      converterTabs: 'Tipo de conversión',
      tabNames: { mp4: 'MP4', mp3: 'MP3', jpg: 'JPG' },
      language: 'Idioma',
      original: 'Original',
      remove: 'Quitar archivo',
      quality: 'Calidad de salida',
      estimate: 'Estimado',
      estimateTitle: 'Peso final estimado; puede variar según el contenido',
      preparing: 'Preparando…',
      ready: 'Listo',
      cancel: 'Cancelar',
      reveal: 'Mostrar en Finder',
      another: 'Convertir otro',
      anotherImage: 'Convertir otra imagen',
      subtitleAdd: '+ Añadir subtítulos SRT',
      subtitleOptional: '(opcional)',
      subtitleRemove: 'Quitar subtítulos',
      processed: 'Procesado',
      analyzing: 'Analizando…',
      compressing: 'Comprimiendo…',
      finishing: 'Finalizando…',
      smaller: 'más pequeño',
      failed: 'La conversión no pudo completarse.',
      folder: 'Elegir carpeta de salida',
      folderChange: 'clic para cambiar · clic derecho para quitar',
      outputTitle: '¿Dónde guardar las conversiones?',
      outputDetail: 'Puedes guardarlas junto a los originales o elegir otra carpeta.',
      outputSame: 'Misma carpeta',
      outputChoose: 'Elegir carpeta…',
      outputCancel: 'Cancelar',
      unsupported: 'El archivo no tiene un formato compatible.',
      busy: 'Ya hay una conversión en curso.',
      ffmpegMissing: 'No se encontró FFmpeg. Instálalo con "brew install ffmpeg".',
      startError: 'No se pudo iniciar FFmpeg',
      converters: {
        mp4: {
          defaultProfile: 'quality',
          dropTitle: 'Convierte videos a MP4',
          dropHint: 'arrastra un video o haz clic para abrir',
          action: 'Convertir a MP4',
          format: 'MP4',
          profiles: [
            { id: 'quality', name: 'Alta calidad', detail: 'Misma resolución' },
            { id: 'light', name: 'Liviana', detail: 'Máx. 720p · audio HQ' }
          ]
        },
        mp3: {
          defaultProfile: 'high',
          dropTitle: 'Arrastra un audio o video',
          dropHint: 'convierte audio o extráelo de un video',
          action: 'Convertir a MP3',
          format: 'MP3',
          profiles: [
            { id: 'high', name: 'Alta calidad', detail: '320 kbps' },
            { id: 'light', name: 'Liviana', detail: '128 kbps' }
          ]
        },
        jpg: {
          defaultProfile: 'high',
          dropTitle: 'Convierte imágenes a JPG',
          dropHint: 'JPG · PNG · WEBP · BMP · TIFF',
          action: 'Convertir a JPG',
          format: 'JPG',
          profiles: [
            { id: 'high', name: 'Alta calidad', detail: 'Dimensiones originales' },
            { id: 'light', name: 'Liviana', detail: 'Máx. 1280 px' }
          ]
        }
      }
    },
    en: {
      converterTabs: 'Conversion type',
      tabNames: { mp4: 'MP4', mp3: 'MP3', jpg: 'JPG' },
      language: 'Language',
      original: 'Original',
      remove: 'Remove file',
      quality: 'Output quality',
      estimate: 'Estimated',
      estimateTitle: 'Estimated final size; may vary depending on the content',
      preparing: 'Preparing…',
      ready: 'Ready',
      cancel: 'Cancel',
      reveal: 'Show in Finder',
      another: 'Convert Another',
      anotherImage: 'Convert More Images',
      subtitleAdd: '+ Add SRT subtitles',
      subtitleOptional: '(optional)',
      subtitleRemove: 'Remove subtitles',
      processed: 'Processed',
      analyzing: 'Analyzing…',
      compressing: 'Compressing…',
      finishing: 'Finishing…',
      smaller: 'smaller',
      failed: 'The conversion could not be completed.',
      folder: 'Choose output folder',
      folderChange: 'click to change · right-click to clear',
      outputTitle: 'Where should conversions be saved?',
      outputDetail: 'Save them next to the originals or choose another folder.',
      outputSame: 'Same folder',
      outputChoose: 'Choose folder…',
      outputCancel: 'Cancel',
      unsupported: 'The file format is not supported.',
      busy: 'A conversion is already in progress.',
      ffmpegMissing: 'FFmpeg was not found. Install it with "brew install ffmpeg".',
      startError: 'Could not start FFmpeg',
      converters: {
        mp4: {
          defaultProfile: 'quality',
          dropTitle: 'Convert videos to MP4',
          dropHint: 'drop a video or click to browse',
          action: 'Convert to MP4',
          format: 'MP4',
          profiles: [
            { id: 'quality', name: 'High quality', detail: 'Same resolution' },
            { id: 'light', name: 'Lightweight', detail: 'Max. 720p · HQ audio' }
          ]
        },
        mp3: {
          defaultProfile: 'high',
          dropTitle: 'Drop an audio file or video',
          dropHint: 'convert audio or extract it from video',
          action: 'Convert to MP3',
          format: 'MP3',
          profiles: [
            { id: 'high', name: 'High quality', detail: '320 kbps' },
            { id: 'light', name: 'Lightweight', detail: '128 kbps' }
          ]
        },
        jpg: {
          defaultProfile: 'high',
          dropTitle: 'Convert images to JPG',
          dropHint: 'JPG · PNG · WEBP · BMP · TIFF',
          action: 'Convert to JPG',
          format: 'JPG',
          profiles: [
            { id: 'high', name: 'High quality', detail: 'Original dimensions' },
            { id: 'light', name: 'Lightweight', detail: 'Max. 1280 px' }
          ]
        }
      }
    }
  };

  const $ = (id) => document.getElementById(id);
  const tab = $('tab-convert');
  const dropZone = $('dropZone');
  const fileCard = $('fileCard');
  const errorMessage = $('errorMessage');
  const convertButton = $('convertButton');
  const cancelButton = $('cancelButton');
  const revealButton = $('revealButton');
  const anotherButton = $('anotherButton');

  const state = {
    language: 'es',
    converter: 'mp4',
    file: null,
    files: [],
    subtitle: null,
    profile: 'quality',
    converting: false,
    cancelRequested: false,
    activeIndex: 0,
    completedCount: 0,
    outputPath: null,
    outputPaths: [],
    outputDirectories: {}
  };

  const FOLDER_SVG = '<svg viewBox="0 0 20 20" aria-hidden="true"><path d="M2.5 5.5a1.5 1.5 0 0 1 1.5-1.5h3.2l1.6 1.8H16a1.5 1.5 0 0 1 1.5 1.5v7.2A1.5 1.5 0 0 1 16 16H4a1.5 1.5 0 0 1-1.5-1.5Z"></path></svg>';

  function outputKey() {
    return `${state.converter}:${state.profile}`;
  }

  let modalResolve = null;

  function copy() {
    return COPY[state.language];
  }

  function currentConverter() {
    return copy().converters[state.converter];
  }

  function currentAction() {
    if (state.files.length > 1) {
      const format = currentConverter().format;
      return state.language === 'es'
        ? `Convertir ${state.files.length} archivos a ${format}`
        : `Convert ${state.files.length} files to ${format}`;
    }
    return currentConverter().action;
  }

  function formatBytes(bytes) {
    if (!Number.isFinite(bytes) || bytes <= 0) return '0 MB';
    const megabytes = bytes / (1024 ** 2);
    return `${megabytes < 1 ? megabytes.toFixed(2) : megabytes.toFixed(1)} MB`;
  }

  function setError(message = '') {
    errorMessage.textContent = message;
    errorMessage.classList.toggle('hidden', !message);
  }

  function resetSubtitle() {
    state.subtitle = null;
    $('subtitleFile').textContent = '';
    $('subtitleFile').classList.add('hidden');
    $('subtitleLabel').classList.remove('hidden');
    $('subtitleRemove').classList.add('hidden');
  }

  function estimatedFileBytes(file, profileId) {
    if (!file || !file.size) return 0;
    const { size, duration = 0, width = 0, height = 0, name = '' } = file;

    if (state.converter === 'mp3') {
      const bitrates = { high: 320000, light: 128000 };
      return duration > 0 ? (duration * bitrates[profileId] / 8) * 1.02 : size * 0.35;
    }

    if (state.converter === 'mp4') {
      if (profileId === 'quality') return size * 0.85;
      const scale = width && height ? Math.min(1, 1280 / width, 720 / height) : 1;
      return size * Math.min(0.65, 0.42 * Math.max(0.18, scale ** 2));
    }

    const extension = name.split('.').pop()?.toLowerCase();
    const isAlreadyJpeg = extension === 'jpg' || extension === 'jpeg';
    const isWebp = extension === 'webp';
    if (profileId === 'high') return size * (isWebp ? 1.2 : isAlreadyJpeg ? 0.9 : 0.58);
    const scale = width && height ? Math.min(1, 1280 / width, 1280 / height) : 1;
    return size * (isWebp ? 0.48 : isAlreadyJpeg ? 0.35 : 0.28) * (scale ** 2);
  }

  function estimatedBytes(profileId) {
    const files = state.files.length ? state.files : state.file ? [state.file] : [];
    return files.reduce((total, file) => total + estimatedFileBytes(file, profileId), 0);
  }

  function renderProfiles() {
    const profileGrid = $('profileGrid');
    profileGrid.replaceChildren();
    for (const profile of currentConverter().profiles) {
      const button = document.createElement('button');
      button.className = `profile${profile.id === state.profile ? ' active' : ''}`;
      button.dataset.profile = profile.id;
      button.setAttribute('aria-pressed', profile.id === state.profile ? 'true' : 'false');

      const name = document.createElement('strong');
      name.textContent = profile.name;
      const detail = document.createElement('small');
      detail.textContent = profile.detail;
      const estimate = document.createElement('span');
      estimate.className = 'profile-estimate';
      estimate.title = copy().estimateTitle;
      const estimateLabel = document.createElement('small');
      estimateLabel.textContent = copy().estimate;
      const estimateValue = document.createElement('strong');
      estimateValue.textContent = state.file ? `≈ ${formatBytes(estimatedBytes(profile.id))}` : '—';
      estimate.append(estimateLabel, estimateValue);

      const customFolder = state.outputDirectories[`${state.converter}:${profile.id}`];
      const folderButton = document.createElement('span');
      folderButton.className = `card-action folder-action profile-folder${customFolder ? ' custom' : ''}`;
      folderButton.innerHTML = FOLDER_SVG;
      folderButton.setAttribute('role', 'button');
      folderButton.setAttribute('aria-label', copy().folder);
      folderButton.title = customFolder
        ? `${customFolder} · ${copy().folderChange}`
        : copy().folder;
      folderButton.addEventListener('click', (event) => {
        event.stopPropagation();
        pickProfileFolder(profile.id);
      });
      folderButton.addEventListener('contextmenu', (event) => {
        event.preventDefault();
        event.stopPropagation();
        delete state.outputDirectories[`${state.converter}:${profile.id}`];
        renderProfiles();
      });

      button.append(name, detail, estimate, folderButton);
      profileGrid.append(button);
    }
    profileGrid.style.setProperty('--profile-count', currentConverter().profiles.length);
  }

  function resetFile() {
    state.file = null;
    state.files = [];
    state.outputPath = null;
    state.outputPaths = [];
    resetSubtitle();
    $('subtitlePicker').classList.add('hidden');
    tab.classList.remove('has-subtitle-option');
    state.profile = currentConverter().defaultProfile;
    tab.classList.remove('is-processing', 'is-complete');
    fileCard.classList.add('hidden');
    $('fileThumbnail').removeAttribute('src');
    $('fileThumbnail').classList.add('hidden');
    $('fileMark').classList.remove('hidden');
    fileCard.classList.remove('has-thumbnail');
    dropZone.classList.remove('hidden');
    $('progressPanel').classList.add('hidden');
    $('resultPanel').classList.add('hidden');
    cancelButton.classList.add('hidden');
    anotherButton.classList.add('hidden');
    revealButton.classList.add('hidden');
    convertButton.classList.remove('hidden');
    convertButton.disabled = true;
    $('profilesSection').classList.remove('hidden');
    convertButton.textContent = currentAction();
    anotherButton.textContent = copy().another;
    renderProfiles();
    setError();
  }

  function selectConverter(converterId) {
    if (!copy().converters[converterId] || state.converting || converterId === state.converter) return;
    state.converter = converterId;
    state.profile = currentConverter().defaultProfile;
    document.querySelectorAll('#tab-convert [data-converter]').forEach((item) => {
      const isActive = item.dataset.converter === converterId;
      item.classList.toggle('active', isActive);
      item.setAttribute('aria-selected', isActive ? 'true' : 'false');
      item.tabIndex = isActive ? 0 : -1;
    });
    $('dropTitle').textContent = currentConverter().dropTitle;
    $('dropHint').textContent = currentConverter().dropHint;
    dropZone.setAttribute('aria-label', currentConverter().dropTitle);
    $('fileMark').dataset.format = currentConverter().format;
    convertButton.textContent = currentConverter().action;
    renderProfiles();
    resetFile();
  }

  function setFile(file, files = [file]) {
    const detectedConverter = file.mediaType === 'audio'
      ? 'mp3'
      : file.mediaType === 'image' ? 'jpg' : (file.mediaType === 'video' && state.converter !== 'mp3' ? 'mp4' : state.converter);
    if (detectedConverter !== state.converter) selectConverter(detectedConverter);

    files = files.filter((item) => {
      if (state.converter === 'mp4') return item.mediaType === 'video';
      if (state.converter === 'mp3') return item.mediaType === 'video' || item.mediaType === 'audio';
      if (state.converter === 'jpg') return item.mediaType === 'image';
      return false;
    });
    if (!files.length) {
      setError(copy().unsupported);
      return;
    }
    file = files[0];

    state.file = file;
    state.files = files;
    state.profile = currentConverter().defaultProfile;
    state.outputPath = null;
    tab.classList.remove('is-processing', 'is-complete');
    $('fileName').textContent = files.length > 1 ? `${file.name}  +${files.length - 1}` : file.name;
    $('fileSize').textContent = formatBytes(files.reduce((total, item) => total + item.size, 0));
    if (file.thumbnail) {
      $('fileThumbnail').src = file.thumbnail;
      $('fileThumbnail').classList.remove('hidden');
      $('fileMark').classList.add('hidden');
      fileCard.classList.add('has-thumbnail');
    } else {
      $('fileThumbnail').removeAttribute('src');
      $('fileThumbnail').classList.add('hidden');
      $('fileMark').classList.remove('hidden');
      fileCard.classList.remove('has-thumbnail');
    }
    fileCard.classList.remove('hidden');
    dropZone.classList.add('hidden');
    const showSubtitlePicker = files.length === 1 && state.converter === 'mp4' && file.mediaType === 'video';
    $('subtitlePicker').classList.toggle('hidden', !showSubtitlePicker);
    tab.classList.toggle('has-subtitle-option', showSubtitlePicker);
    if (!showSubtitlePicker) resetSubtitle();
    $('fileMark').dataset.format = currentConverter().format;
    $('resultPanel').classList.add('hidden');
    anotherButton.classList.add('hidden');
    revealButton.classList.add('hidden');
    convertButton.classList.remove('hidden');
    convertButton.disabled = false;
    convertButton.textContent = currentAction();
    anotherButton.textContent = state.converter === 'jpg' && files.length > 1 ? copy().anotherImage : copy().another;
    renderProfiles();
    setError();
  }

  async function chooseFile() {
    if (state.converting) return;
    try {
      const files = await window.pantoConvert.selectFiles(state.converter);
      if (files.length) setFile(files[0], files);
    } catch (error) {
      setError((error && error.message) || copy().failed);
    }
  }

  async function handleDrop(paths) {
    if (state.converting || !paths || !paths.length) return;
    try {
      const files = await window.pantoConvert.inspectFiles(paths);
      if (files.length) setFile(files[0], files);
      else setError(copy().unsupported);
    } catch (error) {
      setError((error && error.message) || copy().unsupported);
    }
  }

  function setSubtitle(subtitle) {
    state.subtitle = subtitle;
    $('subtitleFile').textContent = subtitle.name;
    $('subtitleFile').classList.remove('hidden');
    $('subtitleLabel').classList.add('hidden');
    $('subtitleRemove').classList.remove('hidden');
    setError();
  }

  async function chooseSubtitle() {
    if (state.converting) return;
    try {
      const subtitle = await window.pantoConvert.selectSubtitle();
      if (subtitle) setSubtitle(subtitle);
    } catch (error) {
      setError((error && error.message) || copy().unsupported);
    }
  }

  function closeOutputModal(result) {
    $('outputModal').classList.add('hidden');
    if (modalResolve) {
      const resolve = modalResolve;
      modalResolve = null;
      resolve(result);
    }
  }

  async function pickProfileFolder(profileId) {
    if (state.converting) return;
    state.profile = profileId;
    renderProfiles();
    try {
      const folder = await window.pantoConvert.chooseFolder();
      if (folder) {
        state.outputDirectories[`${state.converter}:${profileId}`] = folder;
        renderProfiles();
      }
    } catch (error) {
      setError((error && error.message) || copy().failed);
    }
  }

  function chooseOutputLocation() {
    return new Promise((resolve) => {
      modalResolve = resolve;
      $('outputModalTitle').textContent = copy().outputTitle;
      $('outputModalDetail').textContent = copy().outputDetail;
      $('outputSame').textContent = copy().outputSame;
      $('outputChoose').textContent = copy().outputChoose;
      $('outputCancel').textContent = copy().outputCancel;
      $('outputModal').classList.remove('hidden');
    });
  }

  async function pickOutputFolder() {
    try {
      const folder = await window.pantoConvert.chooseFolder();
      closeOutputModal({ canceled: !folder, path: folder || null });
    } catch (error) {
      setError((error && error.message) || copy().failed);
      closeOutputModal({ canceled: true, path: null });
    }
  }

  function setProcessing(active) {
    tab.classList.toggle('is-processing', active);
    convertButton.classList.toggle('hidden', active);
    cancelButton.classList.toggle('hidden', !active);
  }

  async function convert() {
    if (!state.file || state.converting) return;
    const customFolder = state.outputDirectories[outputKey()];
    const destination = customFolder
      ? { canceled: false, path: customFolder }
      : await chooseOutputLocation();
    if (destination.canceled) return;

    state.converting = true;
    state.cancelRequested = false;
    state.completedCount = 0;
    state.outputPaths = [];
    tab.classList.remove('is-complete');
    setProcessing(true);
    setError();
    $('progressPanel').classList.remove('hidden');
    $('progressBar').style.width = '0%';
    $('progressPercent').textContent = '0%';
    $('progressTime').textContent = '';
    $('progressLabel').textContent = copy().preparing;

    try {
      const results = [];
      const errors = [];
      for (let index = 0; index < state.files.length; index += 1) {
        if (state.cancelRequested) break;
        state.activeIndex = index;
        try {
          const result = await window.pantoConvert.convertMedia({
            inputPath: state.files[index].path,
            converter: state.converter,
            profile: state.profile,
            subtitlePath: state.files.length === 1 ? (state.subtitle && state.subtitle.path) || null : null,
            outputDirectory: destination.path
          });
          results.push(result);
          state.outputPaths.push(result.outputPath);
          state.completedCount += 1;
        } catch (error) {
          if (state.cancelRequested) break;
          errors.push(error);
        }
      }
      if (state.cancelRequested) {
        tab.classList.remove('is-processing');
        $('progressPanel').classList.add('hidden');
        cancelButton.classList.add('hidden');
        convertButton.classList.remove('hidden');
        return;
      }
      if (!results.length) throw errors[0] || new Error(copy().failed);
      const result = results[0];
      state.outputPath = result.outputPath;
      tab.classList.remove('is-processing');
      tab.classList.add('is-complete');
      $('progressPanel').classList.add('hidden');
      cancelButton.classList.add('hidden');
      revealButton.classList.remove('hidden');
      revealButton.textContent = copy().reveal;
      const inputBytes = results.reduce((total, item) => total + item.inputBytes, 0);
      const outputBytes = results.reduce((total, item) => total + item.outputBytes, 0);
      const delta = inputBytes ? Math.round((1 - outputBytes / inputBytes) * 100) : 0;
      $('resultPanel').classList.remove('hidden');
      if (state.files.length > 1) {
        $('resultDetails').textContent = `${results.length}/${state.files.length} · ${formatBytes(outputBytes)}`;
        if (errors.length) setError(errors[0].message || copy().failed);
      } else {
        $('resultDetails').textContent = `${formatBytes(result.outputBytes)}${delta > 0 ? ` · ${delta}% ${copy().smaller}` : ''}`;
      }
      if (state.converter === 'jpg') anotherButton.classList.remove('hidden');
      anotherButton.textContent = state.converter === 'jpg' && state.files.length > 1 ? copy().anotherImage : copy().another;
    } catch (error) {
      tab.classList.remove('is-processing', 'is-complete');
      $('progressPanel').classList.add('hidden');
      cancelButton.classList.add('hidden');
      convertButton.classList.remove('hidden');
      setError((error && error.message) || copy().failed);
    } finally {
      state.converting = false;
    }
  }

  async function cancel() {
    state.cancelRequested = true;
    cancelButton.disabled = true;
    try {
      await window.pantoConvert.cancel();
    } finally {
      cancelButton.disabled = false;
    }
  }

  function applyLanguage(language) {
    if (!COPY[language]) return;
    state.language = language;
    $('converterTabs').setAttribute('aria-label', copy().converterTabs);
    document.querySelectorAll('#tab-convert [data-converter]').forEach((item) => {
      item.textContent = copy().tabNames[item.dataset.converter];
    });
    $('dropTitle').textContent = currentConverter().dropTitle;
    $('dropHint').textContent = currentConverter().dropHint;
    dropZone.setAttribute('aria-label', currentConverter().dropTitle);
    $('originalLabel').textContent = copy().original;
    $('removeButton').setAttribute('aria-label', copy().remove);
    $('subtitleLabel').replaceChildren(
      document.createTextNode(copy().subtitleAdd + ' '),
      Object.assign(document.createElement('small'), { textContent: copy().subtitleOptional })
    );
    $('subtitlePicker').setAttribute('aria-label', `${copy().subtitleAdd} ${copy().subtitleOptional}`);
    $('subtitleRemove').setAttribute('aria-label', copy().subtitleRemove);
    $('profilesSection').setAttribute('aria-label', copy().quality);
    $('resultTitle').textContent = copy().ready;
    cancelButton.textContent = copy().cancel;
    anotherButton.textContent = copy().another;
    revealButton.textContent = copy().reveal;
    convertButton.textContent = currentAction();
    if (!state.converting) $('progressLabel').textContent = copy().preparing;
    renderProfiles();
    if (state.file) {
      $('fileName').textContent = state.files.length > 1 ? `${state.file.name}  +${state.files.length - 1}` : state.file.name;
    }
  }

  function wire() {
    $('converterTabs').addEventListener('click', (event) => {
      const item = event.target.closest('[data-converter]');
      if (item) selectConverter(item.dataset.converter);
    });

    $('converterTabs').addEventListener('keydown', (event) => {
      if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key) || state.converting) return;
      const tabs = [...document.querySelectorAll('#tab-convert [data-converter]')];
      const currentIndex = tabs.findIndex((item) => item.dataset.converter === state.converter);
      const nextIndex = event.key === 'Home' ? 0
        : event.key === 'End' ? tabs.length - 1
          : (currentIndex + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
      event.preventDefault();
      selectConverter(tabs[nextIndex].dataset.converter);
      tabs[nextIndex].focus();
    });

    dropZone.addEventListener('click', chooseFile);
    dropZone.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        chooseFile();
      }
    });

    $('subtitlePicker').addEventListener('click', chooseSubtitle);
    $('subtitlePicker').addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        chooseSubtitle();
      }
    });
    $('subtitleRemove').addEventListener('click', (event) => {
      event.stopPropagation();
      resetSubtitle();
    });

    $('removeButton').addEventListener('click', () => { if (!state.converting) resetFile(); });

    $('profileGrid').addEventListener('click', (event) => {
      const button = event.target.closest('[data-profile]');
      if (!button || state.converting) return;
      document.querySelectorAll('#tab-convert [data-profile]').forEach((node) => {
        const isActive = node === button;
        node.classList.toggle('active', isActive);
        node.setAttribute('aria-pressed', isActive ? 'true' : 'false');
      });
      state.profile = button.dataset.profile;
    });

    convertButton.addEventListener('click', convert);
    cancelButton.addEventListener('click', cancel);
    anotherButton.addEventListener('click', resetFile);
    revealButton.addEventListener('click', () => {
      if (state.outputPath) window.pantoConvert.showInFolder(state.outputPath);
    });

    $('outputSame').addEventListener('click', () => closeOutputModal({ canceled: false, path: null }));
    $('outputChoose').addEventListener('click', pickOutputFolder);
    $('outputCancel').addEventListener('click', () => closeOutputModal({ canceled: true, path: null }));
    $('outputModal').addEventListener('click', (event) => {
      if (event.target === $('outputModal')) closeOutputModal({ canceled: true, path: null });
    });
    document.addEventListener('keydown', (event) => {
      if (event.key === 'Escape' && !$('outputModal').classList.contains('hidden')) {
        event.preventDefault();
        closeOutputModal({ canceled: true, path: null });
      }
    });

    window.pantoConvert.onProgress(({ percent, timemark }) => {
      if (!state.converting) return;
      const total = Math.max(1, state.files.length);
      const overallPercent = Math.round(((state.completedCount + (percent / 100)) / total) * 100);
      $('progressBar').style.width = `${overallPercent}%`;
      $('progressPercent').textContent = `${overallPercent}%`;
      $('progressTime').textContent = timemark ? `${copy().processed}: ${timemark}` : '';
      const phase = percent < 20 ? copy().analyzing : percent < 90 ? copy().compressing : copy().finishing;
      $('progressLabel').textContent = state.files.length > 1 ? `${state.activeIndex + 1}/${state.files.length} · ${phase}` : phase;
    });
    window.pantoConvert.onStatus(({ message }) => {
      if (message && state.converting) {
        $('progressLabel').textContent = state.files.length > 1 ? `${state.activeIndex + 1}/${state.files.length} · ${message}` : message;
      }
    });
  }

  function init(language) {
    state.language = COPY[language] ? language : 'es';
    wire();
    resetFile();
    applyLanguage(state.language);
  }

  window.ConvertTab = {
    init,
    setLanguage: applyLanguage,
    handleDrop
  };
})();
