from io import BytesIO
import sys
from moosync_edk import http_request

class MockSSL:
    CERT_NONE = None
    CERT_OPTIONAL = None
    CERT_REQUIRED = None
    OPENSSL_VERSION = "OpenSSL 1.1.1"
    SSLWantReadError = None
    SSLSyscallError = None

    def create_default_context(self, *args, **kwargs):
        return None

    def wrap_socket(self, *args, **kwargs):
        return None

    def get_default_verify_paths(self):
        return None
    
class MockUrlLib():
    def __init__(self):
        pass

    def __name__(self):
        return "urllib3"

def mock_umask(*args, **kwargs):
    return args[0]

sys.modules['ssl'] = MockSSL()
sys.modules['urllib3'] = MockUrlLib()
sys.modules['os'].umask = mock_umask

import os
os.environ["YTDLP_NO_LAZY_EXTRACTORS"] = "true"

import yt_dlp.downloader
# from yt_dlp.downloader.http import HttpFD
# from yt_dlp import YoutubeDL, extractor
# from yt_dlp.extractor import extractors
import yt_dlp
import yt_dlp.utils.networking
import yt_dlp.networking.common
from yt_dlp.networking.common import register_rh, RequestHandler, Features, register_preference, Response
from yt_dlp.networking._helper import InstanceStoreMixin
import urllib.parse
from urllib.request import OpenerDirector
from yt_dlp.utils._utils import parse_iso8601
import yt_dlp.postprocessor
from yt_dlp import YoutubeDL

# Cache all the imports
parse_iso8601("211018")
urllib.parse.urlparse("http://example.com").netloc.encode("idna").decode("ascii")

class CustomResponse(Response):
    def __init__(self, fp, url, headers, status = 200, reason = None, extensions = None):
        super().__init__(fp, url, headers, status, reason, extensions)

@register_rh
class UrllibCustomRH(RequestHandler, InstanceStoreMixin):
    _SUPPORTED_URL_SCHEMES = ('http', 'https', 'data', 'ftp')
    _SUPPORTED_PROXY_SCHEMES = ('http', 'socks4', 'socks4a', 'socks5', 'socks5h')
    _SUPPORTED_FEATURES = (Features.NO_PROXY)
    RH_NAME = 'urllib_custom'

    def __init__(self, *, enable_file_urls: bool = False, **kwargs):
        super().__init__(**kwargs)
        self.enable_file_urls = enable_file_urls

    def _check_extensions(self, extensions):
        super()._check_extensions(extensions)
        extensions.pop('cookiejar', None)
        extensions.pop('timeout', None)
        extensions.pop('legacy_ssl', None)

    def _create_instance(self, proxies, cookiejar, legacy_ssl_support=None):
        return OpenerDirector()

    def _send(self, request):
        if request.data is not None:
            response = http_request(url=request.url, headers=request.headers, method=request.method, body=request.data)
        else:
            response = http_request(url=request.url, headers=request.headers, method=request.method)
        return CustomResponse(fp=BytesIO(response.data_bytes()), url=request.url, headers=request.headers)


@register_preference(UrllibCustomRH)
def requests_preference(rh, request):
    return 1


def normalize_url(url):
    return url

sys.modules['yt_dlp.utils.networking'].normalize_url = normalize_url
yt_dlp.utils.networking.normalize_url = normalize_url
yt_dlp.cookies.normalize_url = normalize_url

ytdl = YoutubeDL({
    'nocheckcertificate': True,
    'verbose': True,
    'outtmpl': "/downloads/ytdl/%(title)s.%(ext)s",
    "paths": {
        "home": "/downloads",
        "temp": "/downloads/temp"
    },
    "cachedir": "/downloads/cache",
    "format": "best/bestaudio",
}, auto_init="no_verbose_header")
