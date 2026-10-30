// Die Navigation bekommt ihren Glaskörper, sobald die Seite sich bewegt hat.
const nav = document.querySelector<HTMLElement>('[data-nav]');
if (nav) {
  const update = () => nav.toggleAttribute('data-scrolled', window.scrollY > 24);
  update();
  window.addEventListener('scroll', update, { passive: true });
}
