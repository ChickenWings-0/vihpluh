/**
 * Duotone icons (DESIGN_SYSTEM §5.2): a gray base shape and one accent detail, flat fills on a
 * 24-unit grid. For section headers and empty states only — never on a button by themselves.
 * Both layers follow the theme and accent through the `duo-base` / `duo-accent` classes.
 */
import type { ReactNode } from "react";

export interface DuoProps {
  size?: number;
}

function Duo({ size = 24, children }: DuoProps & { children: ReactNode }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

/** Display with a play triangle. */
export function VideoDuo(props: DuoProps) {
  return (
    <Duo {...props}>
      <path
        className="duo-base"
        d="M3 5h18a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1zm5 15h8v1.5H8z"
      />
      <path className="duo-accent" d="M10 8.5v6l5-3z" />
    </Duo>
  );
}

/** Disc with a note. */
export function MusicDuo(props: DuoProps) {
  return (
    <Duo {...props}>
      <path
        className="duo-base"
        d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm0 7.5a2.5 2.5 0 1 1 0 5 2.5 2.5 0 0 1 0-5z"
        fillRule="evenodd"
      />
      <path className="duo-accent" d="M15 4.2l5 1.6v2.6l-3.4-1.1v5.9a2.3 2.3 0 1 1-1.6-2.2z" />
    </Duo>
  );
}

/** Globe with a signal arc. */
export function NetworkDuo(props: DuoProps) {
  return (
    <Duo {...props}>
      <path
        className="duo-base"
        d="M11 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zm-1 2.2v4.3H6.3A7 7 0 0 1 10 5.2zm0 6.3v4H5.4a7 7 0 0 1 0-4zm0 6v1.3a7 7 0 0 1-3.7-1.3z"
        fillRule="evenodd"
      />
      <path
        className="duo-accent"
        d="M14 3.3a9 9 0 0 1 7.7 7.7h-2A7 7 0 0 0 14 5.3zm0 4a5 5 0 0 1 3.7 3.7h-2.2A3 3 0 0 0 14 9.5z"
      />
    </Duo>
  );
}

/** Clock face with hands. */
export function RecentDuo(props: DuoProps) {
  return (
    <Duo {...props}>
      <path className="duo-base" d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z" />
      <path className="duo-accent" d="M11 6h2v5.6l3.7 2.1-1 1.7L11 12.8z" />
    </Duo>
  );
}

/** Folder back with its tab. */
export function FolderDuo(props: DuoProps) {
  return (
    <Duo {...props}>
      <path className="duo-base" d="M2 8h20v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z" />
      <path className="duo-accent" d="M2 6a2 2 0 0 1 2-2h5l2 2h9a2 2 0 0 1 2 2v.5H2z" />
    </Duo>
  );
}

/** List lines with a leading bullet. */
export function PlaylistDuo(props: DuoProps) {
  return (
    <Duo {...props}>
      <path className="duo-base" d="M8 5h14v2.5H8zm0 5.8h14v2.5H8zm0 5.7h14V19H8z" />
      <path className="duo-accent" d="M2 4l4.5 2.3L2 8.6zm.3 6.8h3v2.5h-3zm0 5.7h3V19h-3z" />
    </Duo>
  );
}

/** Shelf of spines, one of them picked out. */
export function LibraryDuo(props: DuoProps) {
  return (
    <Duo {...props}>
      <path
        className="duo-base"
        d="M3 4h3.5v14H3zm5.5 0H12v14H8.5zM2 19.5h20V21H2zM18.2 6.3l3.3 11-3.3 1-3.3-11z"
      />
      <path className="duo-accent" d="M13.5 4H16v14h-2.5z" />
    </Duo>
  );
}

/** Two slider tracks with their knobs. */
export function GeneralDuo(props: DuoProps) {
  return (
    <Duo {...props}>
      <path className="duo-base" d="M2 6h20v2.5H2zm0 9.5h20V18H2z" />
      <path className="duo-accent" d="M13 3.5h4v7.5h-4zM6 13h4v7.5H6z" />
    </Duo>
  );
}

/** Window frame with a panel docked on the right. */
export function InterfaceDuo(props: DuoProps) {
  return (
    <Duo {...props}>
      <path
        className="duo-base"
        d="M4 4h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z"
      />
      <path className="duo-accent" d="M15 7h4.5v10H15z" />
    </Duo>
  );
}

/** Keyboard with one key picked out. */
export function ControlsDuo(props: DuoProps) {
  return (
    <Duo {...props}>
      <path
        className="duo-base"
        d="M3 5h18a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2zm1 3v3h3V8zm9.5 0v3h3V8zm4.5 0v3h2V8zM4 13v3h2v-3zm14 0v3h2v-3z"
        fillRule="evenodd"
      />
      <path className="duo-accent" d="M8.75 8h3v3h-3zM8 13h8v3H8z" />
    </Duo>
  );
}

/** Disc with a play triangle. */
export function PlaybackDuo(props: DuoProps) {
  return (
    <Duo {...props}>
      <path className="duo-base" d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z" />
      <path className="duo-accent" d="M9.5 7.5v9l7.5-4.5z" />
    </Duo>
  );
}

/** Caption box with two lines of text. */
export function SubtitlesDuo(props: DuoProps) {
  return (
    <Duo {...props}>
      <path
        className="duo-base"
        d="M4 4h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z"
      />
      <path className="duo-accent" d="M5 11h6v2.3H5zm8 0h6v2.3h-6zM5 15h9v2.3H5zm11 0h3v2.3h-3z" />
    </Duo>
  );
}

/** Gauge with its needle. */
export function PerformanceDuo(props: DuoProps) {
  return (
    <Duo {...props}>
      <path
        className="duo-base"
        d="M12 4a10 10 0 0 1 10 10v4a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1v-4A10 10 0 0 1 12 4z"
      />
      <path className="duo-accent" d="M16.6 7.6l1.6 1.2-4.3 5.9a2.2 2.2 0 1 1-1.7-1.2z" />
    </Duo>
  );
}
