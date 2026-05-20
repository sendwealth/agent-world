"use client";

import { TimelineView } from "@/components/TimelineView";
import { getEmergenceEvents, getTimelineSnapshots } from "@/lib/data";

export default function TimelinePage() {
  const events = getEmergenceEvents();
  const snapshots = getTimelineSnapshots();

  return (
    <div className="mx-auto max-w-5xl px-4 py-8">
      <h1 className="text-2xl font-bold text-white md:text-3xl">文明时间线</h1>
      <p className="mt-2 text-zinc-400">
        拖动滑块或点击事件节点，浏览 5000 Tick 内的文明涌现历程。
      </p>
      <div className="mt-6">
        <TimelineView events={events} snapshots={snapshots} />
      </div>
    </div>
  );
}
