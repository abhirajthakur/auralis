// Rasterize brand SVGs to PNGs using sharp (prebuilt binaries, no system deps).
// Run: node assets/brand/rasterize.mjs
import sharp from "sharp";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, "icons");
mkdirSync(outDir, { recursive: true });

const jobs = [
  { in: "auralis-mark-accent.svg", out: "auralis-1024.png", size: 1024 },
  { in: "auralis-mark-accent.svg", out: "auralis-512.png", size: 512 },
  { in: "auralis-mark-accent.svg", out: "auralis-256.png", size: 256 },
  { in: "auralis-mark-mono.svg", out: "auralis-mono-256.png", size: 256 },
  { in: "auralis-mark-mono.svg", out: "auralis-mono-64.png", size: 64 },
  { in: "auralis-mark-mono.svg", out: "auralis-mono-32.png", size: 32 },
  { in: "auralis-mark-mono.svg", out: "auralis-mono-20.png", size: 20 },
];

for (const j of jobs) {
  await sharp(join(here, j.in))
    .resize(j.size, j.size)
    .png()
    .toFile(join(outDir, j.out));
  console.log("wrote", j.out);
}
console.log("done");
