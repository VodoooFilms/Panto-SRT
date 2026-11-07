'use strict';

// Panto Studio shell: fixed window, three tabs, shared theme and language.
(function () {
  const $ = (id) => document.getElementById(id);
  const TABS = ['convert', 'upscale', 'subtitles'];
  const themeToggle = $('themeToggle');
  const languageToggle = $('languageToggle');

  const state = {
    tab: 'convert',
    language: 'es',
    theme: 'dark'
  };

  function tabCopy() {
    return state.language === 'en'
      ? {
          light: 'Switch to light mode',
          dark: 'Switch to dark mode',
          switchLanguage: 'Switch to Spanish',
          sections: 'Sections',
          tabs: { convert: 'Convert', upscale: 'Upscale', subtitles: 'Subtitles' }
        }
      : {
          light: 'Cambiar a modo claro',
          dark: 'Cambiar a modo oscuro',
          switchLanguage: 'Cambiar a inglés',
          sections: 'Secciones',
          tabs: { convert: 'Convertir', upscale: 'Upscale', subtitles: 'Subtítulos' }
        };
  }

  function applyTheme(theme) {
    state.theme = theme === 'light' ? 'light' : 'dark';
    document.documentElement.dataset.theme = state.theme;
    const isDark = state.theme === 'dark';
    const copy = tabCopy();
    themeToggle.setAttribute('aria-label', isDark ? copy.light : copy.dark);
    themeToggle.title = isDark ? copy.light : copy.dark;
    localStorage.setItem('panto-studio-theme', state.theme);
  }

  function selectTab(tab, persist = true) {
    if (!TABS.includes(tab)) tab = 'convert';
    state.tab = tab;
    for (const name of TABS) {
      const section = $(`tab-${name}`);
      const button = document.querySelector(`.studio-tab[data-tab="${name}"]`);
      const active = name === tab;
      section.classList.toggle('hidden', !active);
      if (button) {
        button.classList.toggle('active', active);
        button.setAttribute('aria-selected', active ? 'true' : 'false');
        button.tabIndex = active ? 0 : -1;
      }
    }
    if (persist) localStorage.setItem('panto-studio-tab', tab);
  }

  function applyLanguage(language) {
    state.language = language === 'en' ? 'en' : 'es';
    document.documentElement.lang = state.language;
    languageToggle.textContent = state.language === 'es' ? 'ES' : 'EN';
    const copy = tabCopy();
    languageToggle.setAttribute('aria-label', copy.switchLanguage);
    languageToggle.title = copy.switchLanguage;
    document.querySelector('.studio-tabs').setAttribute('aria-label', copy.sections);
    for (const name of TABS) {
      const button = document.querySelector(`.studio-tab[data-tab="${name}"]`);
      if (button) button.textContent = copy.tabs[name];
      $(`tab-${name}`).setAttribute('aria-label', copy.tabs[name]);
    }
    applyTheme(state.theme);
    localStorage.setItem('panto-studio-language', state.language);

    if (window.ConvertTab) window.ConvertTab.setLanguage(state.language);
    if (window.UpscaleTab) window.UpscaleTab.setLanguage(state.language);
    if (window.SubtitlesTab) window.SubtitlesTab.setLanguage(state.language);
    if (window.pantoSRT) window.pantoSRT.setLanguage(state.language).catch(() => {});
  }

  function handleDrop(paths) {
    const modules = {
      convert: window.ConvertTab,
      upscale: window.UpscaleTab,
      subtitles: window.SubtitlesTab
    };
    const module = modules[state.tab];
    if (module && module.handleDrop) module.handleDrop(paths);
  }

  function wire() {
    document.querySelector('.studio-tabs').addEventListener('click', (event) => {
      const button = event.target.closest('[data-tab]');
      if (button) selectTab(button.dataset.tab);
    });

    document.querySelector('.studio-tabs').addEventListener('keydown', (event) => {
      if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
      const buttons = [...document.querySelectorAll('.studio-tab[data-tab]')];
      const currentIndex = buttons.findIndex((button) => button.dataset.tab === state.tab);
      const nextIndex = event.key === 'Home' ? 0
        : event.key === 'End' ? buttons.length - 1
          : (currentIndex + (event.key === 'ArrowRight' ? 1 : -1) + buttons.length) % buttons.length;
      event.preventDefault();
      selectTab(buttons[nextIndex].dataset.tab);
      buttons[nextIndex].focus();
    });

    themeToggle.addEventListener('click', () => {
      applyTheme(state.theme === 'dark' ? 'light' : 'dark');
    });

    languageToggle.addEventListener('click', () => {
      applyLanguage(state.language === 'es' ? 'en' : 'es');
    });

    // The header is the native drag handle; controls stay clickable.
    const startDragOnPointerDown = (event) => {
      if (event.button !== 0) return;
      if (event.target.closest('.theme-toggle, .language-toggle, .studio-tabs, .studio-tab')) return;
      if (window.pantoSRT) window.pantoSRT.startWindowDrag();
    };
    document.querySelector('.masthead').addEventListener('pointerdown', startDragOnPointerDown);
    document.querySelector('.window-drag-region').addEventListener('pointerdown', startDragOnPointerDown);
  }

  window.PantoStudio = { handleDrop, selectTab };

  window.addEventListener('DOMContentLoaded', () => {
    const savedTab = localStorage.getItem('panto-studio-tab');
    const savedLanguage = localStorage.getItem('panto-studio-language');
    const savedTheme = localStorage.getItem('panto-studio-theme');

    state.language = savedLanguage === 'en' ? 'en' : 'es';
    applyTheme(savedTheme === 'light' ? 'light' : 'dark');
    wire();

    if (window.ConvertTab) window.ConvertTab.init(state.language);
    if (window.UpscaleTab) window.UpscaleTab.init(state.language);
    if (window.SubtitlesTab) window.SubtitlesTab.init(state.language);

    applyLanguage(state.language);
    selectTab(TABS.includes(savedTab) ? savedTab : 'convert', false);
  });
})();
