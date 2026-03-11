'use client';

import Link from "next/link";
import { usePathname } from "next/navigation";

export function NavLink({
  href,
  icon,
  children,
}: {
  href: string;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  const pathname = usePathname();
  // Exact match for root ("/"), prefix match for all other routes so that
  // e.g. "/hospital/..." still highlights the Simulation link.
  const isActive = href === "/" ? pathname === "/" : pathname.startsWith(href);

  return (
    <Link
      href={href}
      aria-label={typeof children === 'string' ? children : undefined}
      className="group flex items-center gap-2.5 px-3 py-2.5 text-xs font-medium transition-all duration-150"
      style={{
        color: isActive ? "#e2e8f0" : "#475569",
        borderLeft: `2px solid ${isActive ? "#22d3ee" : "transparent"}`,
        borderRadius: "0 4px 4px 0",
        marginLeft: "-1px",
        backgroundColor: isActive ? "rgba(34,211,238,0.07)" : "transparent",
      }}
      onMouseEnter={(e) => {
        if (isActive) return; // active state already handles styling
        const el = e.currentTarget;
        el.style.color = "#f1f5f9";
        el.style.borderLeftColor = "#22d3ee";
        el.style.backgroundColor = "rgba(34,211,238,0.05)";
      }}
      onMouseLeave={(e) => {
        if (isActive) return;
        const el = e.currentTarget;
        el.style.color = "#475569";
        el.style.borderLeftColor = "transparent";
        el.style.backgroundColor = "transparent";
      }}
    >
      <span style={{ color: "inherit", opacity: isActive ? 1 : 0.7 }}>{icon}</span>
      {children}
    </Link>
  );
}
