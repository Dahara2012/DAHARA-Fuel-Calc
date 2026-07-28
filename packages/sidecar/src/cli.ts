import { IRacingSdkAdapter, SidecarRunner } from "./index.ts";
import type { SidecarEvent } from "@dahara/shared";

function emitJsonl(e: SidecarEvent): void {
  try {
    process.stdout.write(JSON.stringify(e) + "\n");
  } catch {
    // ignore broken pipe
  }
}

async function main(): Promise<void> {
  if (process.platform !== "win32") {
    process.stderr.write(
      `[sidecar] iRacing SDK is Windows-only; refusing to run on ${process.platform}.\n`,
    );
    process.exit(1);
  }

  const runner = new SidecarRunner({
    sdkFactory: () => new IRacingSdkAdapter(),
    emit: emitJsonl,
    now: () => Date.now(),
  });

  const shutdown = () => {
    runner.stop();
    process.exit(0);
  };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);

  await runner.run();
  if (!runner.isRunning()) {
    process.exit(1);
  }
}

main().catch((err) => {
  process.stderr.write(`[sidecar] fatal: ${String(err)}\n`);
  process.exit(1);
});
