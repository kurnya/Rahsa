type P = { size?: number };

const base = (size: number) => ({
  width: size,
  height: size,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.8,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
});

export const Search = ({ size = 16 }: P) => (
  <svg {...base(size)}><circle cx="11" cy="11" r="6.5" /><path d="m20 20-4.2-4.2" /></svg>
);
export const LockIcon = ({ size = 16 }: P) => (
  <svg {...base(size)}><rect x="5" y="10.5" width="14" height="10" rx="2" /><path d="M8 10.5V8a4 4 0 0 1 8 0v2.5" /></svg>
);
export const Gear = ({ size = 16 }: P) => (
  <svg {...base(size)}><circle cx="12" cy="12" r="3" /><path d="M12 2.8v2.4M12 18.8v2.4M4.2 7.5l2 1.2M17.8 15.3l2 1.2M4.2 16.5l2-1.2M17.8 8.7l2-1.2" /></svg>
);
export const Trash = ({ size = 16 }: P) => (
  <svg {...base(size)}><path d="M4 7h16M9.5 7V4.5h5V7M6.5 7l1 13h9l1-13" /></svg>
);
export const Plus = ({ size = 16 }: P) => (
  <svg {...base(size)}><path d="M12 5v14M5 12h14" /></svg>
);
export const Dice = ({ size = 16 }: P) => (
  <svg {...base(size)}><rect x="4" y="4" width="16" height="16" rx="3" /><circle cx="9" cy="9" r=".8" fill="currentColor" /><circle cx="15" cy="15" r=".8" fill="currentColor" /><circle cx="15" cy="9" r=".8" fill="currentColor" /><circle cx="9" cy="15" r=".8" fill="currentColor" /></svg>
);
export const Pencil = ({ size = 16 }: P) => (
  <svg {...base(size)}><path d="M4 20h4L19 9l-4-4L4 16z" /><path d="m13.5 6.5 4 4" /></svg>
);
export const Close = ({ size = 16 }: P) => (
  <svg {...base(size)}><path d="M6 6l12 12M18 6 6 18" /></svg>
);
export const Building = ({ size = 16 }: P) => (
  <svg {...base(size)}><path d="M4 20h16M6 20V9l6-4 6 4v11M10 20v-5h4v5" /></svg>
);

/** The Rahsa mark: gold coin with the cut-out R (assets/rahsa-logo). */
export const Mark = ({ className = "mark" }: { className?: string }) => (
  <svg className={className} viewBox="172 172 680 680" aria-hidden="true">
    <defs>
      <linearGradient id="rahsa-gold" gradientUnits="userSpaceOnUse" x1="200" y1="160" x2="824" y2="864">
        <stop offset="0" stopColor="#F5D488" />
        <stop offset="1" stopColor="#C58A2A" />
      </linearGradient>
    </defs>
    <path fill="url(#rahsa-gold)" fillRule="evenodd" d="M172 512A340 340 0 1 0 852 512A340 340 0 1 0 172 512ZM372 343.0A43.0 43.0 0 0 1 458 343.0V681.0A43.0 43.0 0 0 1 372 681.0ZM494 300H588.0A112.0 112.0 0 0 1 588.0 524H494A8 8 0 0 1 486 516V308A8 8 0 0 1 494 300ZM541.3 412.0A40 40 0 1 0 621.3 412.0A40 40 0 1 0 541.3 412.0ZM532.09 559.96A111.1 46 47.6 1 1 681.91 724.04A111.1 46 47.6 1 1 532.09 559.96Z" />
  </svg>
);
