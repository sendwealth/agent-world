import type { Metadata } from "next";
import { DemoNavbar } from "@/components/demo/DemoNavbar";

export const metadata: Metadata = {
  title: "Agent World — Civilizational Emergence Demo",
  description:
    "Watch 50 AI agents build a civilization from scratch. An interactive demo of emergent behavior in multi-agent systems.",
};

export default function DemoLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100 antialiased">
      <DemoNavbar />
      <main className="pt-14">{children}</main>
    </div>
  );
}
