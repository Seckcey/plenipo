import type { ServersSnapshot, ServerView } from "@plenipo/types";

const classes: ServersSnapshot["classes"] = [
  { class: "look", label: "Look around", examples: "ls, cat, df" },
  { class: "services", label: "Start, stop, and restart services", examples: "systemctl restart" },
  { class: "change", label: "Install, deploy, and change files", examples: "git pull" },
  { class: "destroy", label: "Delete, wipe, or shut down", examples: "rm, reboot" },
  { class: "admin", label: "Run as administrator", examples: "sudo" },
  { class: "other", label: "Other commands", examples: "scripts" },
];

export function sampleServer(overrides: Partial<ServerView["server"]> = {}): ServerView {
  const server: ServerView["server"] = {
    id: "srv-shop",
    name: "Shop",
    host: "203.0.113.10",
    port: 22,
    user: "shop",
    environment: "production",
    signIn: "password",
    hostKey: {
      algorithm: "ssh-ed25519",
      fingerprint: "SHA256:AAAAbbbbCCCCddddEEEEffffGGGGhhhhIIIIjjjjKKK",
      pinnedAt: Date.now() - 60_000,
    },
    roles: ["role-ops"],
    classes: ["look", "services"],
    approval: "every",
    folders: ["/var/www/shop"],
    forwards: [],
    createdAt: 0,
    updatedAt: 0,
    ...overrides,
  };
  return {
    server,
    address: `${server.user}@${server.host}:${server.port}`,
    stored: { key: false, passphrase: false, password: true },
    connected: [],
  };
}

export function sampleServers(servers: ServerView[] = [sampleServer()]): ServersSnapshot {
  return {
    servers,
    roles: [
      { id: "role-ops", name: "Operations Engineer", canConnect: true },
      { id: "role-writer", name: "Documentation Writer", canConnect: false },
    ],
    classes,
    vault: { available: true, label: "Windows Credential Manager", stored: [] },
    notices: [],
  };
}
