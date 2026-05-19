interface Env {
  OCT_KV: KVNamespace;
}

interface ConfigBundle {
  schemaVersion: number;
  createdAt: string;
  platform: string;
  files: FileEntry[];
  totalBytes: number;
  bundleHash: string;
}

interface FileEntry {
  path: string;
  contentBase64: string;
  sha256: string;
  size: number;
}

interface MetaInfo {
  schemaVersion: number;
  updatedAt: string;
  fileCount: number;
  totalBytes: number;
  bundleHash: string;
  platform: string;
}

interface ErrorResponse {
  error: {
    code: string;
    message: string;
    details?: Record<string, unknown>;
  };
  requestId: string;
}

const TOKEN_REGEX = /^[A-Za-z0-9]{32,}$/;

function errorResponse(
  code: string,
  message: string,
  status: number,
  requestId: string,
  details?: Record<string, unknown>
): Response {
  const body: ErrorResponse = {
    error: { code, message, details },
    requestId,
  };
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "Content-Type": "application/json",
      "Cache-Control": "no-store",
    },
  });
}

async function getTokenHash(token: string): Promise<string> {
  const encoder = new TextEncoder();
  const data = encoder.encode(token);
  const hashBuffer = await crypto.subtle.digest("SHA-256", data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");
}

function extractToken(request: Request): string | null {
  const auth = request.headers.get("Authorization");
  if (!auth || !auth.startsWith("Bearer ")) return null;
  return auth.slice(7);
}

async function authenticate(
  request: Request,
  env: Env
): Promise<{ tokenHash: string } | Response> {
  const token = extractToken(request);
  if (!token) {
    return errorResponse(
      "UNAUTHORIZED",
      "missing Authorization header",
      401,
      crypto.randomUUID()
    );
  }
  if (!TOKEN_REGEX.test(token)) {
    return errorResponse(
      "INVALID_TOKEN",
      "token must be alphanumeric and at least 32 characters",
      403,
      crypto.randomUUID()
    );
  }
  const tokenHash = await getTokenHash(token);
  return { tokenHash };
}

const INDEX_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Oct - Config Preview</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f172a; color: #e2e8f0; min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 2rem; }
  .container { max-width: 800px; width: 100%; }
  h1 { font-size: 1.5rem; margin-bottom: 1.5rem; color: #f8fafc; }
  .form { display: flex; gap: 0.5rem; margin-bottom: 2rem; }
  input[type="password"] { flex: 1; padding: 0.75rem 1rem; background: #1e293b; border: 1px solid #334155; border-radius: 0.5rem; color: #e2e8f0; font-size: 0.875rem; }
  input[type="password"]:focus { outline: none; border-color: #3b82f6; }
  button { padding: 0.75rem 1.5rem; background: #3b82f6; border: none; border-radius: 0.5rem; color: white; font-weight: 500; cursor: pointer; }
  button:hover { background: #2563eb; }
  .meta { background: #1e293b; border-radius: 0.5rem; padding: 1rem; margin-bottom: 1rem; display: grid; grid-template-columns: repeat(auto-fit, minmax(120px, 1fr)); gap: 0.75rem; }
  .meta-item { display: flex; flex-direction: column; }
  .meta-label { font-size: 0.75rem; color: #94a3b8; }
  .meta-value { font-size: 0.875rem; color: #f8fafc; }
  .file-list { background: #1e293b; border-radius: 0.5rem; overflow: hidden; }
  .file-item { padding: 0.75rem 1rem; border-bottom: 1px solid #334155; cursor: pointer; display: flex; justify-content: space-between; align-items: center; }
  .file-item:last-child { border-bottom: none; }
  .file-item:hover { background: #334155; }
  .file-name { font-size: 0.875rem; font-family: monospace; }
  .file-size { font-size: 0.75rem; color: #94a3b8; }
  .file-content { background: #0f172a; border: 1px solid #334155; border-radius: 0.5rem; padding: 1rem; margin-top: 1rem; max-height: 400px; overflow: auto; font-family: monospace; font-size: 0.8rem; white-space: pre-wrap; word-break: break-all; }
  .error { background: #7f1d1d; color: #fca5a5; padding: 1rem; border-radius: 0.5rem; margin-bottom: 1rem; }
  .hidden { display: none; }
</style>
</head>
<body>
<div class="container">
  <h1>Oct Config Preview</h1>
  <form class="form" id="tokenForm">
    <input type="password" id="token" placeholder="Enter your token" required autocomplete="off" />
    <button type="submit">Preview</button>
  </form>
  <div id="error" class="error hidden"></div>
  <div id="preview" class="hidden">
    <div class="meta" id="meta"></div>
    <div class="file-list" id="fileList"></div>
    <div class="file-content hidden" id="fileContent"></div>
  </div>
</div>
<script>
const sensitiveKeys = ["apikey","api_key","token","secret","password","credential","auth"];
function maskSensitive(content, path) {
  const lower = path.toLowerCase();
  if (sensitiveKeys.some(k => lower.includes(k))) {
    return "[SENSITIVE FILE - content masked]";
  }
  let masked = content;
  for (const key of sensitiveKeys) {
    const re = new RegExp('("' + key + '"\\\\s*:\\\\s*")([^"]*)"', 'gi');
    masked = masked.replace(re, '$1' + '***' + '"');
  }
  return masked;
}
document.getElementById('tokenForm').addEventListener('submit', async (e) => {
  e.preventDefault();
  const token = document.getElementById('token').value;
  const errorEl = document.getElementById('error');
  const previewEl = document.getElementById('preview');
  errorEl.classList.add('hidden');
  previewEl.classList.add('hidden');
  try {
    const resp = await fetch('/preview', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ token })
    });
    if (!resp.ok) {
      const err = await resp.json();
      throw new Error(err.error?.message || 'Request failed');
    }
    const data = await resp.json();
    const meta = document.getElementById('meta');
    meta.innerHTML = [
      ['Files', data.fileCount],
      ['Size', data.totalBytes + ' B'],
      ['Hash', data.bundleHash.substring(0, 16) + '...'],
      ['Updated', data.updatedAt],
      ['Platform', data.platform]
    ].map(([l, v]) => '<div class="meta-item"><span class="meta-label">' + l + '</span><span class="meta-value">' + v + '</span></div>').join('');
    const fileList = document.getElementById('fileList');
    fileList.innerHTML = data.files.map((f, i) =>
      '<div class="file-item" data-idx="' + i + '"><span class="file-name">' + escapeHtml(f.path) + '</span><span class="file-size">' + f.size + ' B</span></div>'
    ).join('');
    fileList.querySelectorAll('.file-item').forEach(el => {
      el.addEventListener('click', () => {
        const f = data.files[parseInt(el.dataset.idx)];
        const contentEl = document.getElementById('fileContent');
        contentEl.textContent = maskSensitive(atob(f.contentBase64), f.path);
        contentEl.classList.remove('hidden');
      });
    });
    previewEl.classList.remove('hidden');
  } catch (err) {
    errorEl.textContent = err.message;
    errorEl.classList.remove('hidden');
  }
});
function escapeHtml(s) { return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;'); }
</script>
</body>
</html>`;

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;
    const requestId = crypto.randomUUID();

    try {
      if (path === "/" && request.method === "GET") {
        return new Response(INDEX_HTML, {
          headers: {
            "Content-Type": "text/html; charset=utf-8",
            "Cache-Control": "no-store",
          },
        });
      }

      if (path === "/preview" && request.method === "POST") {
        const body = await request.json() as { token?: string };
        const token = body?.token;
        if (!token || !TOKEN_REGEX.test(token)) {
          return errorResponse("INVALID_TOKEN", "invalid token", 403, requestId);
        }
        const tokenHash = await getTokenHash(token);
        const configKey = `config:v1:${tokenHash}`;
        const bundle = await env.OCT_KV.get(configKey, "json");
        if (!bundle) {
          return errorResponse("NOT_FOUND", "no config found for this token", 404, requestId);
        }
        const b = bundle as ConfigBundle;
        return new Response(JSON.stringify({
          schemaVersion: b.schemaVersion,
          updatedAt: b.createdAt,
          fileCount: b.files.length,
          totalBytes: b.totalBytes,
          bundleHash: b.bundleHash,
          platform: b.platform,
          files: b.files.map(f => ({
            path: f.path,
            contentBase64: f.contentBase64,
            sha256: f.sha256,
            size: f.size,
          })),
        }), {
          headers: {
            "Content-Type": "application/json",
            "Cache-Control": "no-store",
          },
        });
      }

      if (path === "/api/v1/config" || path === "/api/v1/meta") {
        const authResult = await authenticate(request, env);
        if ("status" in authResult) return authResult;
        const { tokenHash } = authResult;

        if (request.method === "PUT" && path === "/api/v1/config") {
          const bundle = await request.json() as ConfigBundle;

          // Validate bundle structure
          if (!bundle.files || !Array.isArray(bundle.files)) {
            return errorResponse("INVALID_BUNDLE", "files array is required", 400, requestId);
          }
          if (bundle.totalBytes > 10 * 1024 * 1024) {
            return errorResponse("BUNDLE_TOO_LARGE", "bundle exceeds 10MB limit", 400, requestId);
          }

          // Validate each file entry
          for (const file of bundle.files) {
            if (!file.path || typeof file.path !== "string") {
              return errorResponse("INVALID_ENTRY", "each file must have a path", 400, requestId);
            }
            // Path security
            if (file.path.startsWith("/") || file.path.includes("..") || file.path.includes("\\") || (file.path.includes(":") && file.path.length > 1)) {
              return errorResponse("PATH_SECURITY", `unsafe path: ${file.path}`, 400, requestId);
            }
            if (!file.contentBase64 || !file.sha256) {
              return errorResponse("INVALID_ENTRY", "each file must have contentBase64 and sha256", 400, requestId);
            }
          }

          const configKey = `config:v1:${tokenHash}`;
          const metaKey = `meta:v1:${tokenHash}`;

          await env.OCT_KV.put(configKey, JSON.stringify(bundle));

          const meta: MetaInfo = {
            schemaVersion: bundle.schemaVersion,
            updatedAt: bundle.createdAt,
            fileCount: bundle.files.length,
            totalBytes: bundle.totalBytes,
            bundleHash: bundle.bundleHash,
            platform: bundle.platform,
          };
          await env.OCT_KV.put(metaKey, JSON.stringify(meta));

          return new Response(JSON.stringify(meta), {
            headers: {
              "Content-Type": "application/json",
              "Cache-Control": "no-store",
            },
          });
        }

        if (request.method === "GET" && path === "/api/v1/config") {
          const configKey = `config:v1:${tokenHash}`;
          const bundle = await env.OCT_KV.get(configKey, "json");
          if (!bundle) {
            return errorResponse("NOT_FOUND", "no config found", 404, requestId);
          }
          return new Response(JSON.stringify(bundle), {
            headers: {
              "Content-Type": "application/json",
              "Cache-Control": "no-store",
            },
          });
        }

        if (request.method === "GET" && path === "/api/v1/meta") {
          const metaKey = `meta:v1:${tokenHash}`;
          const meta = await env.OCT_KV.get(metaKey, "json");
          if (!meta) {
            return errorResponse("NOT_FOUND", "no metadata found", 404, requestId);
          }
          return new Response(JSON.stringify(meta), {
            headers: {
              "Content-Type": "application/json",
              "Cache-Control": "no-store",
            },
          });
        }
      }

      return new Response("Not Found", { status: 404 });
    } catch (err) {
      return errorResponse(
        "INTERNAL_ERROR",
        "internal server error",
        500,
        requestId,
        { detail: String(err) }
      );
    }
  },
};
