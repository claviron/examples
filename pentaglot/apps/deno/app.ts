let requests = 0;

export default {
  async fetch(request: Request): Promise<Response> {
    const path = new URL(request.url).pathname;
    if (path === "/deno/bench/plain") return new Response("ok\n");
    if (path === "/deno/bench/json") return Response.json({ message: "ok" });
    if (path === "/deno/bench/echo" && request.method === "POST") {
      return new Response(request.body, { headers: { "content-type": "application/octet-stream" } });
    }
    if (path !== "/deno" && path !== "/deno/echo") {
      return new Response("Not Found", { status: 404 });
    }
    return Response.json({
      runtime: "deno", engine: "Deno Core / V8", method: request.method,
      path, body: await request.text(), requests: ++requests,
    });
  },
};
