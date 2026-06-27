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
  const url = `${WORLD_ENGINE_URL}/${slug}${request.nextUrl.search}`;

  // Pass through all client headers intentionally. Filtering to only
  // Authorization + Content-Type would be stricter, but the backend already
  // ignores irrelevant headers (host, connection, accept-encoding, etc.),
  // and a whitelist would need ongoing maintenance as new endpoints are added.
  const res = await fetch(url, {
    headers: Object.fromEntries(request.headers.entries()),
  });

  const headers = new Headers(res.headers);
  headers.delete("content-encoding");
  headers.delete("content-length");
  headers.delete("transfer-encoding");

  if (!res.body) {
    return new NextResponse(null, {
      status: res.status,
      headers,
    });
  }

  return new Response(res.body, {
    status: res.status,
    headers,
  });
}

export async function POST(
  request: NextRequest,
  { params }: { params: Promise<{ path?: string[] | string }> },
) {
  const { path } = await params;
  const segments = typeof path === "string" ? [path] : (path ?? []);
  const slug = segments.join("/");
  const url = `${WORLD_ENGINE_URL}/${slug}${request.nextUrl.search}`;

  // See GET handler above for rationale on full header pass-through.
  const res = await fetch(url, {
    method: "POST",
    body: await request.arrayBuffer(),
    headers: Object.fromEntries(request.headers.entries()),
  });

  const body = await res.arrayBuffer();
  return new NextResponse(body, {
    status: res.status,
    headers: res.headers,
  });
}

export async function PUT(
  request: NextRequest,
  { params }: { params: Promise<{ path?: string[] | string }> },
) {
  const { path } = await params;
  const segments = typeof path === "string" ? [path] : (path ?? []);
  const slug = segments.join("/");
  const url = `${WORLD_ENGINE_URL}/${slug}${request.nextUrl.search}`;

  // See GET handler above for rationale on full header pass-through.
  const res = await fetch(url, {
    method: "PUT",
    body: await request.arrayBuffer(),
    headers: Object.fromEntries(request.headers.entries()),
  });

  const body = await res.arrayBuffer();
  return new NextResponse(body, {
    status: res.status,
    headers: res.headers,
  });
}

export async function DELETE(
  request: NextRequest,
  { params }: { params: Promise<{ path?: string[] | string }> },
) {
  const { path } = await params;
  const segments = typeof path === "string" ? [path] : (path ?? []);
  const slug = segments.join("/");
  const url = `${WORLD_ENGINE_URL}/${slug}${request.nextUrl.search}`;

  // See GET handler above for rationale on full header pass-through.
  const res = await fetch(url, {
    method: "DELETE",
    headers: Object.fromEntries(request.headers.entries()),
  });

  const body = await res.arrayBuffer();
  return new NextResponse(body, {
    status: res.status,
    headers: res.headers,
  });
}
