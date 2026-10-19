export const palette = {
  skyTop: '#0c0b1a',
  skyHorizon: '#221d47',
  ground: '#090812',
  groundDeep: '#06050c',
  limbCore: '#f1edff',
  limbMid: '#a596ff',
  limbEdge: '#5d7dff',
  star: '#f7f5ff',
  halo: '#c9bdff',
};

const S = 1024;
const CX = 512;

function solveCut(cy, r, top, edge) {
  const half = S / 2;
  const edgeY = cy - Math.sqrt(r * r - half * half);
  let lo = 0;
  let hi = 20000;
  for (let i = 0; i < 200; i++) {
    const e = (lo + hi) / 2;
    const y = cy + top + e - Math.sqrt((r + e) ** 2 - half * half);
    if (y > edgeY + edge) lo = e;
    else hi = e;
  }
  const e = (lo + hi) / 2;
  return { cy: cy + top + e, r: r + e };
}

const OPTICAL = {
  full: { planetCy: 1640, top: 74, edge: 12, star: { x: 668, y: 304, r: 30 } },
  medium: { planetCy: 1610, top: 104, edge: 28, star: { x: 684, y: 300, r: 56 } },
  small: { planetCy: 1560, top: 150, edge: 54, star: { x: 700, y: 296, r: 96 } },
};

function geometry(detail) {
  const o = OPTICAL[detail] ?? OPTICAL.full;
  const planetR = 1060;
  return {
    planetCy: o.planetCy,
    planetR,
    cut: solveCut(o.planetCy, planetR, o.top, o.edge),
    star: o.star,
  };
}

export function detailFor(px) {
  if (px <= 32) return 'small';
  if (px <= 96) return 'medium';
  return 'full';
}

function clipShape(container) {
  if (container === 'squircle') return `<rect width="${S}" height="${S}" rx="232" ry="232"/>`;
  if (container === 'circle') return `<circle cx="${CX}" cy="${CX}" r="${CX}"/>`;
  return null;
}

export function markBody({ detail = 'full', container = 'squircle', id = 'v' } = {}) {
  const g = geometry(detail);
  const horizonY = g.planetCy - g.planetR;
  const clip = clipShape(container);
  const full = detail === 'full';
  const soft = detail !== 'small';
  const withSky = container !== 'none';

  const defs = `
    <linearGradient id="${id}-sky" x1="0" y1="0" x2="0" y2="${horizonY + 40}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="${palette.skyTop}"/>
      <stop offset="1" stop-color="${palette.skyHorizon}"/>
    </linearGradient>
    <linearGradient id="${id}-ground" x1="0" y1="${horizonY}" x2="0" y2="${S}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="${palette.ground}"/>
      <stop offset="1" stop-color="${palette.groundDeep}"/>
    </linearGradient>
    <radialGradient id="${id}-limb" cx="${CX}" cy="${horizonY}" r="${full ? 600 : 700}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="${palette.limbCore}"/>
      <stop offset="0.38" stop-color="${palette.limbMid}"/>
      <stop offset="0.82" stop-color="${palette.limbEdge}" stop-opacity="${withSky ? 0.55 : 0.8}"/>
      <stop offset="1" stop-color="${palette.limbEdge}" stop-opacity="${withSky ? 0.25 : 0.6}"/>
    </radialGradient>
    <radialGradient id="${id}-atmo" cx="${CX}" cy="${horizonY + 10}" r="520" gradientUnits="userSpaceOnUse"
      gradientTransform="translate(0 ${horizonY + 10}) scale(1 0.42) translate(0 ${-(horizonY + 10)})">
      <stop offset="0" stop-color="#8f7dff" stop-opacity="0.42"/>
      <stop offset="0.55" stop-color="#6b72ff" stop-opacity="0.12"/>
      <stop offset="1" stop-color="#6b72ff" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="${id}-halo" cx="${g.star.x}" cy="${g.star.y}" r="${g.star.r * 5}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="${palette.halo}" stop-opacity="0.62"/>
      <stop offset="0.22" stop-color="${palette.halo}" stop-opacity="0.2"/>
      <stop offset="0.6" stop-color="${palette.halo}" stop-opacity="0.05"/>
      <stop offset="1" stop-color="${palette.halo}" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="${id}-edge" x1="0" y1="0" x2="0" y2="${S}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#ffffff" stop-opacity="0.22"/>
      <stop offset="0.28" stop-color="#ffffff" stop-opacity="0.04"/>
      <stop offset="1" stop-color="#ffffff" stop-opacity="0.02"/>
    </linearGradient>
    ${clip ? `<clipPath id="${id}-clip">${clip}</clipPath>` : ''}
    <mask id="${id}-limbmask" maskUnits="userSpaceOnUse" x="0" y="0" width="${S}" height="${S}">
      <circle cx="${CX}" cy="${g.planetCy}" r="${g.planetR}" fill="#fff"/>
      <circle cx="${CX}" cy="${g.cut.cy.toFixed(2)}" r="${g.cut.r.toFixed(2)}" fill="#000"/>
    </mask>`;

  const scene = [
    withSky ? `<rect width="${S}" height="${S}" fill="url(#${id}-sky)"/>` : '',
    withSky && soft ? `<rect width="${S}" height="${S}" fill="url(#${id}-atmo)"/>` : '',
    withSky ? `<circle cx="${CX}" cy="${g.planetCy}" r="${g.planetR}" fill="url(#${id}-ground)"/>` : '',
    `<rect width="${S}" height="${S}" fill="url(#${id}-limb)" mask="url(#${id}-limbmask)"/>`,
    soft ? `<circle cx="${g.star.x}" cy="${g.star.y}" r="${g.star.r * 5}" fill="url(#${id}-halo)"/>` : '',
    `<circle cx="${g.star.x}" cy="${g.star.y}" r="${g.star.r}" fill="${withSky ? palette.star : palette.limbMid}"/>`,
  ].join('\n    ');

  const edge =
    clip && full
      ? container === 'squircle'
        ? `<rect x="3" y="3" width="${S - 6}" height="${S - 6}" rx="229" fill="none" stroke="url(#${id}-edge)" stroke-width="6"/>`
        : `<circle cx="${CX}" cy="${CX}" r="${CX - 3}" fill="none" stroke="url(#${id}-edge)" stroke-width="6"/>`
      : '';

  return `<defs>${defs}
  </defs>
  <g${clip ? ` clip-path="url(#${id}-clip)"` : ''}>
    ${scene}
  </g>
  ${edge}`;
}

export function markSvg(opts = {}) {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${S} ${S}" width="${S}" height="${S}">
  ${markBody(opts)}
</svg>
`;
}

export function trayTemplateSvg() {
  const g = geometry('small');
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="112 170 800 560" width="800" height="560">
  <defs>
    <mask id="t-limbmask" maskUnits="userSpaceOnUse" x="0" y="0" width="${S}" height="${S}">
      <circle cx="${CX}" cy="${g.planetCy}" r="${g.planetR}" fill="#fff"/>
      <circle cx="${CX}" cy="${g.cut.cy.toFixed(2)}" r="${g.cut.r.toFixed(2)}" fill="#000"/>
    </mask>
  </defs>
  <rect width="${S}" height="${S}" fill="#000" mask="url(#t-limbmask)"/>
  <circle cx="${g.star.x}" cy="${g.star.y}" r="${g.star.r}" fill="#000"/>
</svg>
`;
}
