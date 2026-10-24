import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { MockBackend } from "../../ipc/mock";
import { useDisc } from "./useDisc";

describe("useDisc — pick, default output, auto-analyze", () => {
  test("choosing a disc sets the path then auto-analyzes", async () => {
    const backend = new MockBackend();
    let analyzed: string | null = null;
    const original = backend.analyze.bind(backend);
    backend.analyze = async (path) => {
      analyzed = path;
      return original(path);
    };
    const { result } = renderHook(() => useDisc(backend));
    await result.current.pickDisc();
    await waitFor(() => expect(result.current.status?.kind).toBe("ok"));
    expect(result.current.inputPath).toBe("/mock/game.iso");
    expect(analyzed).toBe("/mock/game.iso");
  });

  test("setInputPath computes the default output and analyzes", async () => {
    const backend = new MockBackend();
    const { result } = renderHook(() => useDisc(backend));
    await result.current.setInputPath("/mock/mygame.iso");
    await waitFor(() => expect(result.current.status?.kind).toBe("ok"));
    expect(result.current.outputPath).toBe("/mock/mygame [CrownMod].iso");
  });

  test("an analyzed refusal is surfaced as status", async () => {
    const backend = new MockBackend();
    backend.nextAnalyzeError = "this disc is already modded (600 rows carry a crown) — use your original ISO";
    const { result } = renderHook(() => useDisc(backend));
    await result.current.setInputPath("/mock/modded.iso");
    await waitFor(() => expect(result.current.status?.kind).toBe("refused"));
  });
});

test("changing spawn counts reinspects the disc but preserves the chosen output", async () => {
  const backend = new MockBackend();
  const original = backend.analyze.bind(backend);
  const analyze = vi.spyOn(backend, "analyze").mockImplementation(async (path, four) => ({
    ...await original(path), outputSize: four ? 4_700_000_000 : 4_600_000_000,
  }));
  const { result, rerender } = renderHook(({ four }) => useDisc(backend, four), { initialProps: { four: false } });
  await act(() => result.current.setInputPath("/mock/game.iso"));
  await waitFor(() => expect(result.current.status?.outputSize).toBe(4_600_000_000));
  act(() => result.current.setOutputPath("/chosen/out.iso"));
  rerender({ four: true });
  await waitFor(() => expect(result.current.status?.outputSize).toBe(4_700_000_000));
  expect(analyze).toHaveBeenLastCalledWith("/mock/game.iso", true, false, false);
  expect(result.current.outputPath).toBe("/chosen/out.iso");
});

test("an old inspection cannot replace the current spawn-option verdict", async () => {
  const backend = new MockBackend();
  const original = backend.analyze.bind(backend);
  type Verdict = Awaited<ReturnType<typeof backend.analyze>>;
  const pending: Array<(value: Verdict) => void> = [];
  vi.spyOn(backend, "analyze").mockImplementation(() => new Promise(resolve => pending.push(resolve)));
  const { result, rerender } = renderHook(({ four }) => useDisc(backend, four), { initialProps: { four: false } });
  await act(() => result.current.setInputPath("/mock/game.iso"));
  await waitFor(() => expect(pending).toHaveLength(1));
  rerender({ four: true });
  await waitFor(() => expect(pending).toHaveLength(2));
  const verdict = await original("/mock/game.iso");
  await act(async () => pending[1]!(verdict));
  await waitFor(() => expect(result.current.status?.kind).toBe("ok"));
  await act(async () => pending[0]!({ ...verdict, kind: "refused", text: "stale" }));
  expect(result.current.status?.kind).toBe("ok");
  expect(result.current.analyzing).toBe(false);
});

 test("changing loot reinspects output size while keeping spawn counts independent", async () => {
   const backend = new MockBackend();
   const original = backend.analyze.bind(backend);
   const analyze = vi.spyOn(backend, "analyze").mockImplementation(async (path, _four, drops) => ({
     ...await original(path), outputSize: drops ? 4_800_000_000 : 4_600_000_000,
   }));
   const { result, rerender } = renderHook(({ drops }) => useDisc(backend, false, drops), { initialProps: { drops: false } });
   await act(() => result.current.setInputPath("/mock/game.iso"));
   await waitFor(() => expect(result.current.status?.outputSize).toBe(4_600_000_000));
   act(() => result.current.setOutputPath("/chosen/out.iso"));
   rerender({ drops: true });
   await waitFor(() => expect(result.current.status?.outputSize).toBe(4_800_000_000));
   expect(analyze).toHaveBeenLastCalledWith("/mock/game.iso", false, true, false);
   expect(result.current.outputPath).toBe("/chosen/out.iso");
 });
