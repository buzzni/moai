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
