import type { HTMLAttributes } from "react";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";
import { NavItem } from "../NavItem/NavItem";
import { NavSection } from "../NavSection/NavSection";

export type SettingsPage = "profile" | "password" | "api-keys" | "providers" | "users" | "roles";

const GROUPS: { label: string; pages: { page: SettingsPage; label: string; icon: IconName }[] }[] = [
  {
    label: "Account",
    pages: [
      { page: "profile", label: "Profile", icon: "user" },
      { page: "password", label: "Password", icon: "lock" },
      { page: "api-keys", label: "API keys", icon: "key" },
    ],
  },
  {
    label: "Administration",
    pages: [
      { page: "providers", label: "Providers", icon: "cloud" },
      { page: "users", label: "Users", icon: "users" },
      { page: "roles", label: "Roles", icon: "shield" },
    ],
  },
];

export type SettingsNavProps = {
  active: SettingsPage;
  hrefFor: (page: SettingsPage) => string;
  /** Administration shows only to people who may administer. */
  showAdministration?: boolean;
} & HTMLAttributes<HTMLElement>;

/** The Settings rail (D6). */
export function SettingsNav({ active, hrefFor, showAdministration = true, className, ...rest }: SettingsNavProps) {
  return (
    <nav aria-label="Settings" className={cx("flex h-full w-256 shrink-0 flex-col gap-4 bg-shell pr-12 pl-0 pt-12", className)} {...rest}>
      <h2 className="flex h-52 items-center px-14 type-title text-fg">Settings</h2>
      {GROUPS.filter((g) => showAdministration || g.label !== "Administration").map((g) => (
        <NavSection key={g.label} label={g.label}>
          {g.pages.map((p) => (
            <NavItem key={p.page} icon={p.icon} label={p.label} href={hrefFor(p.page)} active={active === p.page} />
          ))}
        </NavSection>
      ))}
    </nav>
  );
}
