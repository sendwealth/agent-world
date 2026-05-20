import { SandboxForm } from "@/components/SandboxForm";

export default function SandboxPage() {
  return (
    <div className="mx-auto max-w-2xl px-4 py-8">
      <h1 className="text-2xl font-bold text-white md:text-3xl">交互式沙盒</h1>
      <p className="mt-2 text-zinc-400">
        创建你自己的 Agent，看看它在虚拟世界中可能经历什么。纯模拟，不连接后端。
      </p>
      <div className="mt-8">
        <SandboxForm />
      </div>
    </div>
  );
}
