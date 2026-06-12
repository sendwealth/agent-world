import { NextRequest, NextResponse } from "next/server";

const WORLD_ENGINE_URL =
  process.env.WORLD_ENGINE_URL ?? "http://127.0.0.1:8080";

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ path?: string[] | string }> },
) {
  const { path } = await params;
  const segments = typeof path === "string" ? [path] : (path ?? []);
  const slug = segments.join("/");
  const url = `${WORLD_ENGINE_URL}/api/v2/${slug}${request.nextUrl.search}`;

  const res = await fetch(url);

  const headers = new Headers(res.headers);
  headers.delete("content-encoding");
  headers.delete("content-length");
  headers.delete("transfer-encoding");

  return new NextResponse(res.body, { status: res.status, headers });
}
