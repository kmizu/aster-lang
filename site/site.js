document.documentElement.classList.add("js");

const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

requestAnimationFrame(() => {
  document.documentElement.classList.add("is-ready");
});

const revealElements = document.querySelectorAll("[data-reveal]");
const traceStages = document.querySelectorAll("[data-trace-stage]");

if (reduceMotion || !("IntersectionObserver" in window)) {
  revealElements.forEach((element) => element.classList.add("is-visible"));
  traceStages.forEach((element) => element.classList.add("is-active"));
} else {
  const revealObserver = new IntersectionObserver(
    (entries, observer) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        entry.target.classList.add("is-visible");
        observer.unobserve(entry.target);
      });
    },
    { rootMargin: "0px 0px -10%", threshold: 0.08 },
  );

  revealElements.forEach((element) => revealObserver.observe(element));

  const traceObserver = new IntersectionObserver(
    (entries, observer) => {
      if (!entries.some((entry) => entry.isIntersecting)) return;
      traceStages.forEach((stage, index) => {
        window.setTimeout(() => stage.classList.add("is-active"), index * 140);
      });
      observer.disconnect();
    },
    { threshold: 0.25 },
  );

  const trace = document.querySelector(".hero-trace");
  if (trace) traceObserver.observe(trace);
}
