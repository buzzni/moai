document.querySelectorAll('.copy').forEach(function (b) {
  b.addEventListener('click', function () {
    var done = function () { b.textContent = '복사됨'; setTimeout(function () { b.textContent = '복사'; }, 1400); };
    if (navigator.clipboard) navigator.clipboard.writeText(b.dataset.copy).then(done, function () {});
  });
});
