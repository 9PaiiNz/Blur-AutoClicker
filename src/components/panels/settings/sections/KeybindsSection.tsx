import type { Settings } from "../../../../store";
import KeyCaptureInput from "../../../KeyCaptureInput";
import { SettingsCard } from "./shared";

interface Props {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
}

const PAGES = [
  { key: "keybindSimple" as const, label: "Simple" },
  { key: "keybindAdvanced" as const, label: "Advanced" },
  { key: "keybindZones" as const, label: "Zones" },
  { key: "keybindClickPoints" as const, label: "Click Points" },
  { key: "keybindSettings" as const, label: "Settings" },
];

const TOGGLE_GROUPS: {
  title: string;
  items: { key: keyof Settings; label: string; sublabel: string }[];
}[] = [
  {
    title: "Advanced",
    items: [
      {
        key: "keybindMode",
        label: "Click Mode",
        sublabel: "Cycle Toggle / Hold.",
      },
      {
        key: "keybindInputType",
        label: "Input Type",
        sublabel: "Cycle Mouse / Keyboard.",
      },
      {
        key: "keybindDoubleClick",
        label: "Double Click",
        sublabel: "Toggle on or off.",
      },
      {
        key: "keybindDutyCycleMode",
        label: "Duty Cycle",
        sublabel: "Cycle Click / Hold.",
      },
      {
        key: "keybindSpeedRandomization",
        label: "Speed Randomization",
        sublabel: "Toggle on or off.",
      },
      {
        key: "keybindLimits",
        label: "Limits",
        sublabel: "Toggle the active click/time limit.",
      },
    ],
  },
  {
    title: "Zones",
    items: [
      {
        key: "keybindCornerStop",
        label: "Corner Stop",
        sublabel: "Toggle on or off.",
      },
      {
        key: "keybindEdgeStop",
        label: "Edge Stop",
        sublabel: "Toggle on or off.",
      },
      {
        key: "keybindStopZones",
        label: "Custom Stop Zones",
        sublabel: "Toggle on or off.",
      },
    ],
  },
  {
    title: "Click Points",
    items: [
      {
        key: "keybindToggleClickPoints",
        label: "Click Points",
        sublabel: "Toggle on or off.",
      },
      {
        key: "keybindStopWhenComplete",
        label: "Stop When Complete",
        sublabel: "Toggle on or off.",
      },
    ],
  },
];

export default function KeybindsSection({ settings, update }: Props) {
  return (
    <>
      <SettingsCard
        title="Keybinds"
        description="Set a keyboard shortcut for each page. Press the key you want to bind."
      >
        {PAGES.map((page) => (
          <div className="settings-row" key={page.key}>
            <div className="settings-label-group">
              <span className="settings-label">{page.label}</span>
              <span className="settings-sublabel">
                Switch to the {page.label} page.
              </span>
            </div>
            <KeyCaptureInput
              className="settings-keybind-capture"
              value={settings[page.key]}
              onChange={(key) => update({ [page.key]: key })}
            />
          </div>
        ))}
      </SettingsCard>

      {TOGGLE_GROUPS.map((group) => (
        <SettingsCard
          key={group.title}
          title={`${group.title} Toggle Keybinds`}
          description="Bind a key to flip a setting from anywhere in the app. Empty by default."
        >
          {group.items.map((item) => (
            <div className="settings-row" key={item.key}>
              <div className="settings-label-group">
                <span className="settings-label">{item.label}</span>
                <span className="settings-sublabel">{item.sublabel}</span>
              </div>
              <KeyCaptureInput
                className="settings-keybind-capture"
                value={settings[item.key] as string}
                onChange={(key) => update({ [item.key]: key })}
              />
            </div>
          ))}
        </SettingsCard>
      ))}
    </>
  );
}
