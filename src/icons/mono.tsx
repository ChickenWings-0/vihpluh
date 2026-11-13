/**
 * vihpluh's own monochrome glyphs (DESIGN_SYSTEM §5.1), drawn on lucide's 24-unit grid with the
 * same stroke so they sit next to stock icons. More arrive with the features that need them.
 */
import type { SVGProps } from "react";

interface GlyphProps {
  size?: number;
  strokeWidth?: number;
  fill?: string;
  "aria-hidden"?: boolean | "true" | "false";
}

function Svg({
  size = 24,
  strokeWidth = 1.5,
  children,
  ...rest
}: GlyphProps & SVGProps<SVGSVGElement>) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      {...rest}
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

/** Stacked rows with a play mark on the first. */
export function QueueGlyph({ fill: _fill, ...props }: GlyphProps) {
  return (
    <Svg {...props}>
      <path d="M4 6l4 2.5L4 11z" fill="currentColor" />
      <path d="M11 8.5h9M4 15h16M4 19.5h16" />
    </Svg>
  );
}

/** A frame with its right third split off. */
export function SidebarRightGlyph({ fill: _fill, ...props }: GlyphProps) {
  return (
    <Svg {...props}>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M15 4v16" />
    </Svg>
  );
}

/** A frame with a small window in the lower-right corner. */
export function PipGlyph({ fill: _fill, ...props }: GlyphProps) {
  return (
    <Svg {...props}>
      <path d="M21 11V6a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h5" />
      <rect x="13" y="13" width="8" height="7" rx="1.5" />
    </Svg>
  );
}

/** Four open corners: everything but the picture is gone. */
export function ZenGlyph({ fill: _fill, ...props }: GlyphProps) {
  return (
    <Svg {...props}>
      <path d="M4 9V5h4M16 5h4v4M20 15v4h-4M8 19H4v-4" />
    </Svg>
  );
}
