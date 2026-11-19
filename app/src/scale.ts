// The interface is designed at 1600x900 and scaled as a whole to fit the window, so it reads the
// same on a small laptop, a 4K screen, or a 32:9 ultrawide. Wider windows than 16:9 get more
// room; they don't get stretched. CSS reads the scale as --z (see styles.css).

const BASE_W = 1600;
const BASE_H = 900;
let scale = 1;

export function uiScale(): number {
  return scale;
}

export function fitToWindow() {
  const apply = () => {
    const z = Math.min(window.innerHeight / BASE_H, window.innerWidth / BASE_W);
    scale = Math.max(0.6, Math.min(4, Math.round(z * 1000) / 1000)) || 1;
    document.documentElement.style.setProperty("--z", String(scale));
  };
  apply();
  window.addEventListener("resize", apply);
}
