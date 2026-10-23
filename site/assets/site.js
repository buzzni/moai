document.querySelectorAll('.copy').forEach(function (b) {
  var label = b.textContent;
  b.addEventListener('click', function () {
    var done = function () { b.textContent = b.dataset.copied || label; setTimeout(function () { b.textContent = label; }, 1400); };
    if (navigator.clipboard) navigator.clipboard.writeText(b.dataset.copy).then(done, function () {});
  });
});

// 손으로 고른 언어는 기억한다. 루트의 자동 선택(index.html <head>)이 이것을 브라우저 언어보다 먼저 본다.
document.querySelectorAll('.lang a[data-lang]').forEach(function (a) {
  a.addEventListener('click', function () {
    try { localStorage.setItem('moai-lang', a.dataset.lang); } catch (e) {}
  });
});

// 히어로 회전판. 넘기는 시계는 따로 두지 않고 진행 막대의 CSS 애니메이션이 끝나는 때를 쓴다 —
// 마우스를 올리거나 키보드 포커스가 들면 CSS 가 막대를 멈추니 넘기는 것도 함께 멈추고,
// 움직임을 줄인 환경에서는 막대가 돌지 않아 스스로 넘기지 않는다.
(function () {
  var root = document.querySelector('.rotator');
  if (!root) return;
  var tabs = Array.prototype.slice.call(root.querySelectorAll('.rot-tab'));
  var slides = tabs.map(function (t) { return document.getElementById(t.getAttribute('aria-controls')); });
  function show(i, focus) {
    tabs.forEach(function (t, k) {
      var on = k === i;
      t.setAttribute('aria-selected', on ? 'true' : 'false');
      t.tabIndex = on ? 0 : -1;
      slides[k].hidden = !on;
      slides[k].classList.toggle('is-on', on);
    });
    if (focus) tabs[i].focus();
  }
  tabs.forEach(function (t, k) {
    t.addEventListener('click', function () { show(k); });
    t.addEventListener('animationend', function (e) {
      if (e.animationName === 'rot-fill' && t.getAttribute('aria-selected') === 'true') show((k + 1) % tabs.length);
    });
    t.addEventListener('keydown', function (e) {
      var to = { ArrowRight: k + 1, ArrowLeft: k - 1, Home: 0, End: tabs.length - 1 }[e.key];
      if (to === undefined) return;
      e.preventDefault();
      show((to + tabs.length) % tabs.length, true);
    });
  });
})();

// 장면 녹화. 화면에 들어올 때 처음 불러 틀고, 나가면 멈춘다 — 첫 화면을 여는 데 녹화 몇 MB 를 기다리게
// 하지 않으려는 것이다. 움직임을 줄인 환경에서는 저절로 틀지 않고 재생 단추만 둔다.
(function () {
  var reduce = window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches;
  var casts = Array.prototype.slice.call(document.querySelectorAll('.cast[data-cast]'));
  var clips = Array.prototype.slice.call(document.querySelectorAll('video.clip[data-src]'));
  function cast(el) {
    if (!el._player && window.AsciinemaPlayer) {
      el._player = AsciinemaPlayer.create(el.dataset.cast, el, {
        cols: Number(el.dataset.cols), rows: Number(el.dataset.rows), fit: 'width', loop: true,
        autoPlay: !reduce, controls: reduce, idleTimeLimit: 2, poster: 'npt:0:6', theme: 'moai', speed: Number(el.dataset.speed || 1),
        terminalFontFamily: 'ui-monospace, "SF Mono", "JetBrains Mono", Menlo, Consolas, "DejaVu Sans Mono", monospace'
      });
      return null;
    }
    return el._player || null;
  }
  function clip(v) {
    if (!v.getAttribute('src')) v.src = v.dataset.src;
    if (reduce) { v.controls = true; return; }
    var p = v.play(); if (p && p.catch) p.catch(function () {});
  }
  if (!('IntersectionObserver' in window)) { casts.forEach(cast); clips.forEach(clip); return; }
  var seen = new IntersectionObserver(function (entries) {
    entries.forEach(function (e) {
      var el = e.target;
      if (el.tagName === 'VIDEO') { if (e.isIntersecting) clip(el); else el.pause(); return; }
      // 처음 지켜보기 시작할 때도 항목이 한 번 온다 — 화면 밖의 녹화를 그때 불러 틀지 않게, 안 보이면 멈추기만 한다
      if (!e.isIntersecting) { if (el._player) el._player.pause(); return; }
      var player = cast(el);
      if (player && !reduce) player.play();
    });
  }, { threshold: 0.35 });
  casts.concat(clips).forEach(function (el) { seen.observe(el); });
})();
