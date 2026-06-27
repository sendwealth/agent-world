import { NextRequest, NextResponse } from "next/server";

const WORLD_ENGINE_URL =
  process.env.WORLD_ENGINE_URL ?? "http://127.0.0.1:8080";

function buildUpstreamUrl(
  request: NextRequest,
  path: string[] | string | undefined,
): string {
  const segments = typeof path === "string" ? [path] : (path ?? []);
  const slug = segments.join("/");
  return `${WORLD_ENGINE_URL}/api/v2/${slug}${request.nextUrl.search}`;
}

function cleanHeaders(src: Headers): Headers {
  const headers = new Headers(src);
  headers.delete("content-encoding");
  headers.delete("content-length");
  headers.delete("transfer-encoding");
  return headers;
}

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ path?: string[] | string }> },
) {
  const { path } = await params;
  const url = buildUpstreamUrl(request, path);

  // v2 does not forward client headers — the backend does not need them
  // for these read-only endpoints. If auth is required in the future,
  // forward only Authorization + Content-Type explicitly.
  const res = await fetch(url);

  const headers = cleanHeaders(res.headers);

  if (!res.body) {
    return new NextResponse(null, {
      status: res.status,
      headers,
    });
  }

  return new NextResponse(res.body, {
    status: res.status,
    headers,
  });
}

export async function POST(
  request: NextRequest,
  { params }: { params: Promise<{ path?: string[] | string }> },
) {
  const { path } = await params;
  const url = buildUpstreamUrl(request, path);

  // See GET handler above for rationale on no header pass-through.
  const res = await fetch(url, {
    method: "POST",
    body: await request.arrayBuffer(),
  });

  const headers = cleanHeaders(res.headers);
  const body = await res.arrayBuffer();
  return new NextResponse(body, {
    status: res.status,
    headers,
  });
}

export async function PUT(
  request: NextRequest,
  { params }: { params: Promise<{ path?: string[] | string }> },
) {
  const { path } = await params;
  const url = buildUpstreamUrl(request, path);

  // See GET handler above for rationale on no header pass-through.
  const res = await fetch(url, {
    method: "PUT",
    body: await request.arrayBuffer(),
  });

  const headers = cleanHeaders(res.headers);
  const body = await res.arrayBuffer();
  return new NextResponse(body, {
    status: res.status,
    headers,
  });
}

export async function DELETE(
  request: NextRequest,
  { params }: { params: Promise<{ path?: string[] | string }> },
) {
  const { path } = await params;
  const url = buildUpstreamUrl(request, path);

  // See GET handler above for rationale on no header pass-through.
  const res = await fetch(url, {
    method: "DELETE",
  });

  const headers = cleanHeaders(res.headers);
  const body = await res.arrayBuffer();
  return new NextResponse(body, {
    status: res.status,
    headers,
  });
}
