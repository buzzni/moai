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
