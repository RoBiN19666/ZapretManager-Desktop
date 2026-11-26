using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Net;
using System.Net.Security;
using System.Net.Sockets;
using System.Security.Authentication;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;

namespace ZapretManager
{
    public sealed class SiteProbe
    {
        public string Input = "";
        public string Host = "";
        public string Url = "";
        public bool DnsOk;
        public string[] Ips = new string[0];
        public string DnsError = "";
        public bool TcpOk;
        public int TcpMs;
        public string TcpError = "";
        public bool TlsOk;
        public int TlsMs;
        public string TlsError = "";
        public bool HttpOk;
        public int HttpStatus;
        public string HttpError = "";
        /// <summary>ok | dpi | dns | tls | http | down</summary>
        public string Verdict = "";
        public string VerdictText = "";
        public List<string> Related = new List<string>();
        public string HtmlTitle = "";
    }

    /// <summary>
    /// Проверка сайта по этапам: DNS → TCP:443 → TLS с нужным SNI → HTTP.
    /// Если DNS работает, а TCP/TLS рвётся (reset/таймаут) — это типичная подпись ТСПУ.
    /// </summary>
    public static class SiteDoctor
    {
        private static readonly string[] CommonSubdomains = new string[] { "www", "m", "api", "static", "cdn", "i" };

        public static string NormalizeInput(string input, out string url)
        {
            url = "";
            if (string.IsNullOrEmpty(input)) return "";
            string t = input.Trim().Trim('"', '\'', '<', '>');
            if (t.Length == 0) return "";
            if (!t.Contains("://")) t = "https://" + t;
            Uri uri;
            if (!Uri.TryCreate(t, UriKind.Absolute, out uri)) return "";
            if (string.IsNullOrEmpty(uri.Host)) return "";
            url = uri.Scheme + "://" + uri.Host + (uri.IsDefaultPort ? "" : ":" + uri.Port) + "/";
            string host = uri.Host.ToLowerInvariant();
            try
            {
                // Unicode-домен → punycode: DNS/SNI и списки должны видеть ASCII-форму.
                host = new System.Globalization.IdnMapping().GetAscii(host);
            }
            catch { }
            return host;
        }

        public static SiteProbe Check(string input, Action<string> progress)
        {
            SiteProbe p = new SiteProbe();
            p.Input = input;
            string url;
            p.Host = NormalizeInput(input, out url);
            p.Url = url;
            if (p.Host.Length == 0)
            {
                p.Verdict = "down";
                p.VerdictText = "Не удалось разобрать адрес. Введите домен или ссылку, например instagram.com.";
                return p;
            }

            if (progress != null) progress("Определяем адрес " + p.Host + "…");
            Stopwatch sw = Stopwatch.StartNew();
            try
            {
                IPAddress[] addresses = Resolve(p.Host, 5000);
                List<string> v4 = new List<string>();
                List<string> v6 = new List<string>();
                foreach (IPAddress a in addresses)
                {
                    if (a.AddressFamily == AddressFamily.InterNetwork) v4.Add(a.ToString());
                    else if (a.AddressFamily == AddressFamily.InterNetworkV6) v6.Add(a.ToString());
                }
                List<string> all = new List<string>(v4);
                all.AddRange(v6);
                p.Ips = all.ToArray();
                p.DnsOk = all.Count > 0;
                if (!p.DnsOk) p.DnsError = "нет A/AAAA-записей";
            }
            catch (Exception ex)
            {
                p.DnsOk = false;
                p.DnsError = Describe(ex);
            }

            if (!p.DnsOk)
            {
                p.Verdict = "dns";
                p.VerdictText = "Домен не резолвится: " + p.DnsError + ". Это либо опечатка, либо DNS-блокировка — тогда помогает запись в hosts.";
                return p;
            }

            if (progress != null) progress("Проверяем TCP-соединение…");
            p.TcpOk = TcpConnect(p.Host, 443, 5000, out p.TcpMs, out p.TcpError);

            if (p.TcpOk)
            {
                if (progress != null) progress("Проверяем TLS-рукопожатие (SNI «" + p.Host + "»)…");
                p.TlsOk = TlsHandshake(p.Host, 7000, out p.TlsMs, out p.TlsError);
            }

            if (p.TlsOk)
            {
                if (progress != null) progress("Запрашиваем страницу…");
                p.HttpOk = HttpGet(p.Host, 12000, out p.HttpStatus, out p.HttpError, out p.HtmlTitle);
            }

            p.Verdict = Decide(p);
            p.VerdictText = VerdictText(p);
            return p;
        }

        private static string Decide(SiteProbe p)
        {
            if (!p.DnsOk) return "dns";
            if (!p.TcpOk)
            {
                string e = (p.TcpError ?? "").ToLowerInvariant();
                if (e.Contains("таймаут") || e.Contains("reset") || e.Contains("сброс") || e.Contains("forcibly")) return "dpi";
                if (e.Contains("refused") || e.Contains("отказано")) return "down";
                return "down";
            }
            if (!p.TlsOk)
            {
                string e = (p.TlsError ?? "").ToLowerInvariant();
                if (e.Contains("reset") || e.Contains("сброс") || e.Contains("таймаут") || e.Contains("forcibly") || e.Contains("eof")) return "dpi";
                return "tls";
            }
            if (!p.HttpOk) return "http";
            return "ok";
        }

        private static string VerdictText(SiteProbe p)
        {
            switch (p.Verdict)
            {
                case "ok":
                    return "Сайт открывается напрямую: соединение, TLS и HTTP отвечают. Скорее всего, обход для него не нужен — но если контент не грузится, блокируются отдельные адреса (CDN, API) — их можно добавить ниже.";
                case "dpi":
                    return "Похоже на блокировку ТСПУ: DNS отвечает, а соединение рвётся (reset/таймаут) на этапе TCP или TLS с этим доменом. Такое лечится добавлением домена в списки zapret.";
                case "dns":
                    return "Домен не резолвится. Если сайт должен работать — попробуйте прописать адрес в hosts (менеджер может сам подставить IP, если найдёт его через другой DNS).";
                case "tls":
                    return "TCP проходит, но TLS не поднимается. Это может быть блокировка по SNI или проблема сертификата/сервера.";
                case "http":
                    return "Соединение и TLS в порядке, но сервер ответил ошибкой " + p.HttpStatus + ". Это не блокировка провайдера.";
                default:
                    return "Сайт недоступен по сетевой причине; обход DPI вряд ли поможет.";
            }
        }

        /// <summary>Публичная обёртка: похоже ли, что домен блокируют (reset/таймаут на TCP/TLS).</summary>
        public static bool LooksBlockedPublic(string host, out string reason)
        {
            reason = "";
            int ms;
            string err;
            if (!TcpConnect(host, 443, 2500, out ms, out err))
            {
                reason = "TCP: " + err;
                return err.IndexOf("таймаут", StringComparison.OrdinalIgnoreCase) >= 0
                    || err.IndexOf("reset", StringComparison.OrdinalIgnoreCase) >= 0
                    || err.IndexOf("сброс", StringComparison.OrdinalIgnoreCase) >= 0;
            }
            int tlsMs;
            string tlsError;
            if (!TlsHandshake(host, 4000, out tlsMs, out tlsError))
            {
                reason = "TLS: " + tlsError;
                string e = (tlsError ?? "").ToLowerInvariant();
                return e.Contains("reset") || e.Contains("сброс") || e.Contains("таймаут")
                    || e.Contains("eof") || e.Contains("forcibly") || e.Contains("закрыт");
            }
            return false;
        }

        public static bool TcpConnect(string host, int port, int timeoutMs, out int ms, out string error)
        {
            ms = 0;
            error = "";
            Stopwatch sw = Stopwatch.StartNew();
            TcpClient client = new TcpClient();
            try
            {
                IAsyncResult ar = client.BeginConnect(host, port, null, null);
                if (!ar.AsyncWaitHandle.WaitOne(timeoutMs))
                {
                    error = "таймаут " + timeoutMs + " мс";
                    return false;
                }
                client.EndConnect(ar);
                ms = (int)sw.ElapsedMilliseconds;
                return true;
            }
            catch (Exception ex)
            {
                ms = (int)sw.ElapsedMilliseconds;
                error = Describe(ex);
                return false;
            }
            finally
            {
                try { client.Close(); } catch { }
                sw.Stop();
            }
        }

        public static bool TlsHandshake(string host, int timeoutMs, out int ms, out string error)
        {
            ms = 0;
            string err = "";
            int elapsed = 0;
            bool ok = false;
            TcpClient client = new TcpClient();
            Thread thread = new Thread(delegate()
            {
                Stopwatch sw = Stopwatch.StartNew();
                try
                {
                    IAsyncResult ar = client.BeginConnect(host, 443, null, null);
                    if (!ar.AsyncWaitHandle.WaitOne(Math.Max(1000, timeoutMs - 500)))
                    {
                        err = "таймаут TCP";
                        return;
                    }
                    client.EndConnect(ar);
                    using (SslStream ssl = new SslStream(client.GetStream(), false, delegate { return true; }))
                    {
                        // SslProtocols.Default на .NET 4.8 = Ssl3|Tls, что современные системы уже запрещают
                        ssl.AuthenticateAsClient(host, null, SslProtocols.Tls12 | (SslProtocols)12288, false);
                        ok = true;
                        elapsed = (int)sw.ElapsedMilliseconds;
                    }
                }
                catch (Exception ex)
                {
                    err = Describe(ex);
                    elapsed = (int)sw.ElapsedMilliseconds;
                }
                finally
                {
                    try { client.Close(); } catch { }
                    sw.Stop();
                }
            });
            thread.IsBackground = true;
            thread.Start();
            if (!thread.Join(timeoutMs))
            {
                // закрываем сокет снаружи — поток не останется висеть на Connect/Authenticate
                try { client.Close(); } catch { }
                error = "таймаут TLS " + timeoutMs + " мс";
                ms = timeoutMs;
                return false;
            }
            error = err;
            ms = elapsed;
            return ok;
        }

        public static bool HttpGet(string host, int timeoutMs, out int status, out string error, out string title)
        {
            status = 0;
            error = "";
            title = "";
            Web.EnableModernTls();
            HttpWebRequest req = (HttpWebRequest)WebRequest.Create("https://" + host + "/");
            req.Method = "GET";
            req.Proxy = null;
            req.Timeout = timeoutMs;
            req.ReadWriteTimeout = timeoutMs;
            req.AllowAutoRedirect = true;
            req.MaximumAutomaticRedirections = 5;
            req.UserAgent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";
            req.Accept = "text/html,application/xhtml+xml";
            try
            {
                using (HttpWebResponse resp = (HttpWebResponse)req.GetResponse())
                {
                    status = (int)resp.StatusCode;
                    using (StreamReader reader = new StreamReader(resp.GetResponseStream(), Encoding.UTF8))
                    {
                        char[] buffer = new char[262144];
                        int read = reader.Read(buffer, 0, buffer.Length);
                        string html = read > 0 ? new string(buffer, 0, read) : "";
                        Title(html, out title);
                    }
                    return status >= 200 && status < 400;
                }
            }
            catch (WebException ex)
            {
                HttpWebResponse resp = ex.Response as HttpWebResponse;
                if (resp != null)
                {
                    status = (int)resp.StatusCode;
                    error = "HTTP " + status + " " + resp.StatusDescription;
                }
                else
                {
                    error = Describe(ex);
                }
                return false;
            }
            catch (Exception ex)
            {
                error = Describe(ex);
                return false;
            }
        }

        private static void Title(string html, out string title)
        {
            title = "";
            try
            {
                Match m = Regex.Match(html ?? "", "<title[^>]*>(.*?)</title>", RegexOptions.IgnoreCase | RegexOptions.Singleline);
                if (m.Success) title = Regex.Replace(m.Groups[1].Value, "\\s+", " ").Trim();
            }
            catch { }
        }

        /// <summary>Связанные домены: сама страница, её поддомены и упомянутые в HTML хосты.</summary>
        public static List<string> Discover(string input, SiteProbe probe, Action<string> progress)
        {
            List<string> result = new List<string>();
            string dummyUrl;
            string host = probe != null && probe.Host.Length > 0 ? probe.Host : NormalizeInput(input, out dummyUrl);
            if (host.Length == 0) return result;

            AddOnce(result, host);
            string root = Registrable(host);
            if (root.Length > 0 && !string.Equals(root, host, StringComparison.OrdinalIgnoreCase)) AddOnce(result, root);

            foreach (string sub in CommonSubdomains)
            {
                string candidate = sub + "." + root;
                if (!string.Equals(candidate, host, StringComparison.OrdinalIgnoreCase)) AddOnce(result, candidate);
            }

            string html = FetchHtml(host);
            if (html.Length > 0)
            {
                if (progress != null) progress("Ищем адреса в странице…");
                foreach (string found in ExtractHosts(html))
                {
                    string h = found.ToLowerInvariant();
                    if (h.Length < 4 || h.Length > 80) continue;
                    if (h.EndsWith(".local") || h.EndsWith(".lan")) continue;
                    if (h.Contains("w3.org") || h.Contains("schema.org") || h.StartsWith("fonts.")) continue;
                    if (IsIpLike(h)) continue;
                    AddOnce(result, h);
                    if (result.Count >= 40) break;
                }
            }

            if (progress != null) progress("Проверяем связанные адреса…");
            List<string> live = new List<string>();
            List<string> broken = new List<string>();
            foreach (string h in result.ToArray())
            {
                IPAddress[] ips;
                try { ips = Resolve(h, 2000); }
                catch { continue; }
                if (ips == null || ips.Length == 0) continue;
                int ms;
                string err;
                bool tcp = TcpConnect(h, 443, 1500, out ms, out err);
                string tlsErr = "";
                if (tcp)
                {
                    int tlsMs;
                    bool tls = TlsHandshake(h, 2500, out tlsMs, out tlsErr);
                    if (tls) live.Add(h);
                    else broken.Add(h);
                }
                else
                {
                    broken.Add(h);
                }
            }

            List<string> ordered = new List<string>();
            ordered.AddRange(broken);
            ordered.AddRange(live);
            if (probe != null) probe.Related = ordered;
            return ordered;
        }

        private static string FetchHtml(string host)
        {
            Web.EnableModernTls();
            try
            {
                HttpWebRequest req = (HttpWebRequest)WebRequest.Create("https://" + host + "/");
                req.Method = "GET";
                req.Proxy = null;
                req.Timeout = 12000;
                req.ReadWriteTimeout = 12000;
                req.UserAgent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";
                using (HttpWebResponse resp = (HttpWebResponse)req.GetResponse())
                using (StreamReader reader = new StreamReader(resp.GetResponseStream(), Encoding.UTF8))
                {
                    char[] buffer = new char[524288];
                    int read = reader.Read(buffer, 0, buffer.Length);
                    return read > 0 ? new string(buffer, 0, read) : "";
                }
            }
            catch
            {
                return "";
            }
        }

        private static IEnumerable<string> ExtractHosts(string html)
        {
            List<string> hosts = new List<string>();
            try
            {
                foreach (Match m in Regex.Matches(html, "(?:https?:)?//([a-z0-9][a-z0-9._-]{2,80})[/\\\"'? )]", RegexOptions.IgnoreCase))
                {
                    hosts.Add(m.Groups[1].Value.TrimEnd('.'));
                }
                foreach (Match m in Regex.Matches(html, "\\\"([a-z0-9][a-z0-9-]{1,40}(?:\\.[a-z0-9-]{1,40}){1,3}\\.[a-z]{2,8})\\\"", RegexOptions.IgnoreCase))
                {
                    hosts.Add(m.Groups[1].Value);
                }
            }
            catch { }
            return hosts;
        }

        private static bool IsIpLike(string host)
        {
            IPAddress ip;
            return IPAddress.TryParse(host, out ip);
        }

        /// <summary>Грубая оценка «основного» домена: последние две метки (с учётом частых двухуровневых зон).</summary>
        public static string Registrable(string host)
        {
            if (string.IsNullOrEmpty(host)) return "";
            string[] parts = host.Split('.');
            if (parts.Length <= 2) return host;
            string last = parts[parts.Length - 1];
            string prev = parts[parts.Length - 2];
            string[] twoLevel = new string[] { "co.uk", "org.uk", "com.br", "com.au", "co.jp", "co.kr", "com.tr", "co.in", "com.mx" };
            foreach (string t in twoLevel)
            {
                if ((prev + "." + last) == t)
                {
                    return parts.Length >= 3 ? parts[parts.Length - 3] + "." + t : host;
                }
            }
            return prev + "." + last;
        }

        private static void AddOnce(List<string> list, string value)
        {
            foreach (string item in list)
            {
                if (string.Equals(item, value, StringComparison.OrdinalIgnoreCase)) return;
            }
            list.Add(value);
        }

        /// <summary>DNS-резолв с таймаутом: системный резолвер иначе может висеть десятки секунд.</summary>
        public static IPAddress[] Resolve(string host, int timeoutMs)
        {
            try
            {
                IAsyncResult ar = Dns.BeginGetHostAddresses(host, null, null);
                try
                {
                    if (!ar.AsyncWaitHandle.WaitOne(timeoutMs)) return new IPAddress[0];
                    return Dns.EndGetHostAddresses(ar);
                }
                finally
                {
                    try { ar.AsyncWaitHandle.Close(); } catch { }
                }
            }
            catch { return new IPAddress[0]; }
        }

        public static string Describe(Exception ex)
        {
            if (ex == null) return "неизвестная ошибка";
            Exception inner = ex;
            while (inner.InnerException != null) inner = inner.InnerException;
            string text = inner.Message;
            if (inner is SocketException)
            {
                SocketError code = ((SocketException)inner).SocketErrorCode;
                switch (code)
                {
                    case SocketError.TimedOut: return "таймаут";
                    case SocketError.ConnectionReset: return "соединение сброшено (reset)";
                    case SocketError.ConnectionRefused: return "в соединении отказано";
                    case SocketError.HostUnreachable: return "хост недоступен";
                    case SocketError.NetworkUnreachable: return "сеть недоступна";
                }
            }
            return text;
        }

        // ---------- применение ----------

        /// <summary>Добавляет домены в пользовательский список (list-general-user.txt), без повторов.</summary>
        public static string AddToUserList(ZapretInstall install, IEnumerable<string> hosts, out int added, out string path)
        {
            added = 0;
            path = "";
            if (install == null || !install.IsValid) return "Папка zapret не найдена.";
            string listsDir = Path.Combine(install.Root, "lists");
            if (!Directory.Exists(listsDir)) Directory.CreateDirectory(listsDir);
            path = Path.Combine(listsDir, "list-general-user.txt");

            lock (ListChanges.ListFileLock)
            {
                HashSet<string> existing = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
                string text = File.Exists(path) ? TextFile.Read(path) : "";
                foreach (string line in text.Replace("\r\n", "\n").Split('\n'))
                {
                    string norm = ListAudit.Normalize(line);
                    if (norm.Length > 0) existing.Add(norm);
                }

                StringBuilder append = new StringBuilder();
                foreach (string raw in hosts)
                {
                    string host = (raw ?? "").Trim().ToLowerInvariant();
                    if (host.Length == 0) continue;
                    if (host.StartsWith("*.")) host = host.Substring(2);
                    if (host.StartsWith("^")) host = host.Substring(1);
                    if (!ListAudit.IsHostLike(host)) continue;
                    if (!existing.Add(host)) continue;
                    append.AppendLine(host);
                    added++;
                }
                if (added == 0) return "Все эти адреса уже есть в списке.";

                if (text.Length > 0 && !text.EndsWith("\n")) text += "\r\n";
                ZapretOps.WriteTextAtomic(path, text + append);
            }
            return string.Format("Добавлено адресов: {0}.", added);
        }

        /// <summary>Прописывает IP для доменов в системный hosts (с резервной копией).</summary>
        public static string AddToHosts(IEnumerable<string> hosts, out int added, out string backup)
        {
            added = 0;
            backup = "";
            List<string> lines = new List<string>();
            foreach (string raw in hosts)
            {
                string host = (raw ?? "").Trim().ToLowerInvariant();
                if (host.Length == 0) continue;
                if (host.StartsWith("^")) host = host.Substring(1);
                if (!ListAudit.IsHostLike(host)) continue;
                IPAddress[] ips;
                try { ips = Resolve(host, 3000); }
                catch { continue; }
                string ipv4 = "";
                foreach (IPAddress ip in ips)
                {
                    if (ip.AddressFamily == AddressFamily.InterNetwork) { ipv4 = ip.ToString(); break; }
                }
                if (ipv4.Length == 0) continue;
                lines.Add(ipv4 + " " + host);
            }
            if (lines.Count == 0) return "Не удалось определить IP-адреса для этих доменов.";

            string path = HostsPath;
            if (!File.Exists(path)) return "Файл hosts не найден.";
            string content = TextFile.Read(path);
            content = RemoveHostsBlocks(content);
            backup = path + ".zapret-backup-" + DateTime.Now.ToString("yyyyMMdd-HHmmss");
            File.Copy(path, backup, true);

            StringBuilder append = new StringBuilder();
            bool any = false;
            foreach (string line in lines)
            {
                string host = line.Substring(line.IndexOf(' ') + 1);
                if (Regex.IsMatch(content, "(?im)^\\s*[0-9.]+\\s+" + Regex.Escape(host) + "\\s*$")) continue;
                if (!any)
                {
                    append.AppendLine();
                    append.AppendLine("# >>> Zapret Manager: адреса сайтов (" + DateTime.Now.ToString("yyyy-MM-dd HH:mm") + ")");
                    any = true;
                }
                append.AppendLine(line);
                added++;
            }
            if (!any) return "Все эти адреса уже есть в hosts.";
            append.AppendLine("# <<< Zapret Manager");
            if (!content.EndsWith("\n")) content += "\r\n";
            ZapretOps.WriteTextAtomic(path, content + append);
            try { Proc.Run("ipconfig", "/flushdns", 8000); } catch { }
            return string.Format("Записей в hosts добавлено: {0}.", added);
        }

        /// <summary>Убирает прежние блоки менеджера из hosts, чтобы записи не дублировались.</summary>
        private static string RemoveHostsBlocks(string content)
        {
            if (string.IsNullOrEmpty(content)) return content;
            if (content.IndexOf("# >>> Zapret Manager: адреса сайтов", StringComparison.Ordinal) < 0) return content;
            string[] lines = content.Replace("\r\n", "\n").Split('\n');
            StringBuilder sb = new StringBuilder();
            bool inBlock = false;
            foreach (string line in lines)
            {
                string t = line.TrimStart();
                if (!inBlock && t.StartsWith("# >>> Zapret Manager: адреса сайтов", StringComparison.Ordinal))
                {
                    inBlock = true;
                    continue;
                }
                if (inBlock)
                {
                    if (t.StartsWith("# <<< Zapret Manager", StringComparison.Ordinal)) inBlock = false;
                    continue;
                }
                sb.AppendLine(line);
            }
            return sb.ToString();
        }

        public static string HostsPath
        {
            get { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System), "drivers\\etc\\hosts"); }
        }

    }
}
