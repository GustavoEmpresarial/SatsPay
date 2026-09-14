(function () {
  try {
    var raw = localStorage.getItem('satspay-theme');
    var theme = 'light';
    if (raw) {
      var parsed = JSON.parse(raw);
      var t = parsed && parsed.state && parsed.state.theme;
      if (t === 'dark' || t === 'black' || t === 'light') theme = t;
    }
    var root = document.documentElement;
    root.classList.remove('dark', 'theme-black');
    if (theme === 'dark') root.classList.add('dark');
    else if (theme === 'black') root.classList.add('dark', 'theme-black');
  } catch (e) {}
})();
