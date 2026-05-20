import type { Metadata } from "next";
import { Navbar } from "@/components/Navbar";
import "./globals.css";

export const metadata: Metadata = {
  title: "Agent World — Civilizational Emergence Demo",
  description:
    "Watch 50 AI agents build a civilization from scratch. An interactive demo of emergent behavior in multi-agent systems.",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="dark">
      <body className="min-h-screen bg-zinc-950 text-zinc-100 antialiased">
        <Navbar />
        <main className="pt-14">{children}</main>
      </body>
    </html>
  );
}
