// 화면 언어. 한국어 원문을 열쇠로 쓰고, 영어일 때만 아래 표에서 바꾼다.
// 저장된 선택이 없으면 시스템 언어가 한국어일 때 한국어, 아니면 영어.
(function () {
  let lang = null;
  try { lang = localStorage.getItem('tw-lang'); } catch (e) {}
  if (lang !== 'ko' && lang !== 'en') lang = (navigator.language || 'en').toLowerCase().startsWith('ko') ? 'ko' : 'en';
  window.LANG = lang;

  const EN = {
    // 머리글·공통
    'DeskDusk': 'DeskDusk', '설정': 'Settings', '닫기': 'Close', '쉬는 중': 'Idle',
    '사용량을 읽는 중이에요': 'Reading usage…', '사용량을 읽지 못했어요. 잠시 뒤 다시 시도해요.': 'Could not read usage. Will retry shortly.',
    '세션 {0}개 작업 중': '{0} active session(s)', '{0} 갱신': 'updated {0}',
    '{0} {1} 한도가 곧 초기화돼요': '{0} {1} limit resets soon', '{0} {1} 한도가 {2} 뒤 초기화돼요': '{0} {1} limit resets in {2}',
    '오늘 {0} 토큰을 태웠어요': '{0} tokens burned today', '오늘 {0}': 'Today {0}',
    // 카드
    '지금 풍경': 'Scene now', '사용 한도': 'Usage limits', '오늘 태운 토큰': 'Tokens today', '토큰 리듬': 'Token rhythm',
    '최근 1시간': 'last hour', '1시간 전': '1h ago', '30분 전': '30m ago', '지금': 'now',
    '오늘 어디서 태웠나': 'Where it went today', '새 입력': 'New input', '캐시': 'Cache', '출력': 'Output',
    'API로 썼다면': 'If billed via API', '추정': 'est.', 'API 환산 {0}': 'API equiv. {0}', 'API 환산 비용': 'API-equivalent cost',
    '구독료가 아니에요. 같은 양을 API로 썼다면 들었을 정가예요.': 'Not your subscription fee. This is the list price if the same usage went through the API.',
    '구독료 아님 · API 정가 기준 · 요금표 {0}': 'not a subscription fee · API list price · rates as of {0}', '최근 7일': 'Last 7 days', '7일 {0}': '7 days {0}', '오늘': 'Today',
    '분당 {0}': '{0}/min', '캐시 비중 {0}': 'cache share {0}', '오늘은 아직 태운 토큰이 없어요.': 'No tokens burned yet today.',
    // 한도
    '미연결': 'Not connected', '준비 중': 'Coming soon', '{0}를 설치하면 여기에 표시돼요.': 'Install {0} to see it here.',
    '{0} 사용량은 아직 읽지 못해요.': '{0} usage is not supported yet.', '초기화됐어요. 다음 사용 때 갱신돼요': 'Reset. Updates on next use',
    '초기화까지 {0}': 'resets in {0}', '초기화 시각 정보가 없어요': 'No reset time available',
    '한도 값이 아직 없어요. 오늘 {0} 토큰': 'No limit data yet. {0} tokens today', '{0} 기준 · 오늘 {1} 토큰': 'as of {0} · {1} tokens today',
    // 예측
    '이 속도라면': 'At this pace', '· 추정값': '· estimate', '계산 중이에요': 'Calculating…', '도달': 'Full', '주의': 'Warning', '여유': 'OK',
    '{0} {1} 한도가 다 찼어요': '{0} {1} limit is full', '초기화까지 {0} · {1}': 'resets in {0} · {1}',
    '{0} {1} 한도가 거의 다 찼어요': '{0} {1} limit is almost full', '{0} 기준 {1} · 초기화까지 {2}': '{1} as of {0} · resets in {2}',
    '{0} {1} 한도가 {2}쯤 도달해요': '{0} {1} limit will be hit around {2}', '초기화({0})보다 {1} 빨라요.': '{1} before it resets ({0}).',
    '지금 속도면 한도까지 여유가 있어요': 'At this pace you will stay under the limits', '한도 값이 아직 없어요': 'No limit data yet',
    'CLI를 한 번 쓰면 값이 기록돼요.': 'Use the CLI once to record a value.',
    // 풍경 글자
    '이른 아침': 'Early morning', '한낮': 'Midday', '늦은 오후': 'Late afternoon', '해질녘': 'Dusk', '밤': 'Night',
    '고요': 'Calm', '맑음': 'Clear', '구름': 'Cloudy', '가랑비': 'Drizzle', '비': 'Rain', '눈발': 'Flurries', '함박눈': 'Heavy snow',
    '뿌연 하늘': 'Hazy', '황사': 'Dust', '황사 바람': 'Dust storm', '먹구름': 'Dark clouds', '소나기': 'Showers', '뇌우': 'Thunderstorm',
    '한도 {0} · {1}': 'Limit {0} · {1}', '한도 정보가 없어 실제 시각 기준이에요': 'No limit data, so this follows the real time of day',
    '미리보기 중이에요': 'Previewing',
    // 설정
    '풍경': 'Scene', '도시': 'City', '바다': 'Sea', '벚꽃': 'Blossom', '호수': 'Lake', '네온': 'Neon', '궁궐': 'Palace', '첨성대': 'Observatory', '개발자': 'Developer', '디자이너': 'Designer', '작가': 'Writer', '카페': 'Café',
    '빨리 태울 때의 날씨': 'Weather when burning fast', '날씨': 'Weather', '계절 따라': 'By season', '눈': 'Snow',
    '표시할 서비스': 'Services to show', '표시할 서비스(여러 개 선택)': 'Services to show (multi-select)', '서비스 선택': 'Services',
    '표시할 카드': 'Cards to show', '시계': 'Clock', '어디서 태웠나': 'Where it went', '그림 배경': 'Painted background',
    '로그인할 때 자동 실행': 'Launch at login', '메뉴바에 표시': 'Menu bar shows', '서비스마다': 'Each service', '가장 많이 찬 한도': 'Fullest limit',
    '알림': 'Notifications', '한도 경고 알림 (80%, 95%)': 'Limit alerts (80%, 95%)', '언어': 'Language',
    '풍경 미리보기': 'Scene preview', '값의 출처': 'Data sources', '카드 위치·크기 처음으로': 'Reset card layout', '카드 고정': 'Lock cards',
    '카드는 끌어서 옮기고, 오른쪽 아래 모서리를 끌어 크기를 바꿀 수 있어요.': 'Drag cards to move them; drag the bottom-right corner to resize.',
    '한도가 찰수록 해가 지고, 빨리 태울수록 날씨가 거칠어져요.': 'The fuller your limits, the later the day; the faster you burn, the rougher the weather.',
    '한도 사용률': 'Limit used', '태우는 속도': 'Burn rate', '실제 값으로 돌아가기': 'Back to real values',
    '이 컴퓨터의 기록만 읽어요': 'Reads only local logs',
    '기본 설정에서는 로그인 정보를 읽지 않고 인터넷으로 보내는 것도 없어요. 한도 값은 각 도구를 마지막으로 쓴 시점의 값이라, 그 뒤로 안 썼다면 실제와 다를 수 있어요.':
      'By default no credentials are read and nothing is sent over the network. Limit values are from the last time you used each tool, so they may be stale.',
    'Claude 한도는 터미널에서 Claude Code를 쓸 때 새 값으로 바뀌어요.': 'Claude limits update when you use Claude Code in a terminal.',
    // 값의 출처
    'Claude 토큰': 'Claude tokens', 'Claude 한도': 'Claude limits', 'Codex 토큰': 'Codex tokens', 'Codex 한도': 'Codex limits',
    '세션 기록 · {0}': 'session logs · {0}', '상태줄 · {0}': 'status line · {0}', '기록 없음': 'no logs', '상태줄 값 없음': 'no status line data',
    '값 없음': 'no data', '지원 준비 중': 'coming soon', '설치 안 됨': 'not installed', '소진 예측': 'Pace estimate',
    '지금까지 평균 속도로 추정': 'from average pace so far', '작업 중 세션': 'Active sessions', '최근 3분 안에 기록된 세션 수': 'logs written in the last 3 min',
    '비용에서 빠진 모델': 'Models without a price', '없음': 'none',
    // 시작하기
    '시작하기': 'Get started', '30초면 끝나요': 'takes 30 seconds', '바탕화면에 위젯을 띄웠어요': 'Your widget is on the desktop',
    '다른 창 위에 작게 떠 있어요. 바탕화면에 깔아 두려면 위젯을 오른쪽 클릭해 「항상 위에 두기」를 끄거나, 메뉴바 메뉴의 「위젯을 다른 창 위에 띄우기」를 끄세요. 모양과 내용도 오른쪽 클릭으로 바꿔요.': 'It floats above other windows. To keep it on the desktop instead, right-click the widget and turn off "Keep on top", or turn off "Float widgets above windows" in the menu bar menu. Right-click to change its shape and contents too.',
    'Claude 한도를 정확하게 볼까요?': 'Want exact Claude limits?',
    '켜면 이 컴퓨터의 Claude Code 로그인 정보로 Anthropic 서버에 한도를 물어요. 켤 때 키체인 접근을 물어볼 수 있어요. 끄면 로컬 기록만 읽어요.': 'When on, DeskDusk asks Anthropic for your limits using the Claude Code login on this computer. You may be asked to allow keychain access. When off, only local logs are read.',
    '컴퓨터를 켤 때 같이 켤까요?': 'Start with your computer?', '로그인할 때 창 없이 위젯과 메뉴바 요약만 올라와요.': 'At login, only the widget and menu bar summary appear, with no window.',
    '이 창은 닫아도 돼요. 메뉴바의 DeskDusk 아이콘에서 다시 열 수 있고, 모두 「설정」에서 나중에 바꿀 수 있어요.': 'You can close this window. Reopen it from the DeskDusk menu bar icon; everything can be changed later in Settings.',
    '끌어서 옮기기 · 오른쪽 클릭으로 꾸미기': 'Drag to move · Right-click to customize',
    // 연결 상태
    '연결 상태': 'Connections', '로그인하지 않아요': 'no sign-in', 'Codex 토큰과 한도': 'Codex tokens and limits',
    'DeskDusk는 기본 설정에서 Claude나 OpenAI에 접속하지 않아요. Claude Code와 Codex가 쓸 때마다 이 컴퓨터에 남기는 기록 파일을 읽을 뿐이에요.':
      'By default DeskDusk never connects to Claude or OpenAI. It only reads the log files that Claude Code and Codex write on this computer as you use them.',
    '준비 없이 읽어요. Claude Code가 남긴 세션 기록에서 가져와요.': 'Works with no setup. Read from the session logs Claude Code writes.',
    '준비 없이 읽어요. Codex가 남긴 세션 기록에서 가져와요.': 'Works with no setup. Read from the session logs Codex writes.',
    'Claude Code를 쓰면 여기에 표시돼요.': 'Shows up once you use Claude Code.', 'Codex를 쓰면 여기에 표시돼요.': 'Shows up once you use Codex.',
    '연결됨. 터미널에서 Claude Code를 쓸 때마다 새 값으로 바뀌어요.': 'Connected. Updates whenever you use Claude Code in a terminal.',
    '연결됨. 터미널에서 Claude Code를 한 번 쓰면 값이 들어와요.': 'Connected. Use Claude Code in a terminal once to get the first value.',
    '다른 상태줄을 쓰고 있어 자동으로 연결하지 않아요. README의 안내대로 직접 연결할 수 있어요.': 'You already use another status line, so this is not set up automatically. See the README to connect by hand.',
    'Claude Code가 설치되어 있지 않아요.': 'Claude Code is not installed.',
    '아직 연결되지 않았어요. Claude Code는 한도 값을 기록에 남기지 않아서 한 번 연결이 필요해요.': 'Not connected yet. Claude Code does not write limits to its logs, so this needs a one-time setup.',
    '연결하려면 Node.js가 필요해요.': 'Node.js is required to connect.',
    '연결 끊기': 'Disconnect', 'Claude 한도 연결하기': 'Connect Claude limits', '앱에서만 동작해요': 'Works in the app only',
    '연결했어요. 터미널에서 Claude Code를 한 번 쓰면 값이 들어와요.': 'Connected. Use Claude Code in a terminal once to get the first value.',
    '연결을 끊었어요.': 'Disconnected.',
    '연결하면 Claude 설정 파일(~/.claude/settings.json)의 상태줄 항목 하나만 바꾸고, 바꾸기 전에 같은 폴더에 백업을 남겨요.':
      'Connecting changes only the status line entry in ~/.claude/settings.json and leaves a backup next to it first.',
    '설정의 「연결 상태」에서 연결할 수 있어요.': 'Connect it under Settings → Connections.',
    // 정확한 Claude 한도 가져오기(선택 기능)
    '정확한 Claude 한도 가져오기': 'Fetch exact Claude limits', '서버에서 직접': 'from server', '서버 직접 조회 · {0}': 'fetched from server · {0}',
    '켜면 이 컴퓨터에 저장된 Claude Code 로그인 정보로 Anthropic 서버에 한도를 10분마다 물어요. 켤 때 키체인 접근을 물어볼 수 있어요. 로그인 정보는 요청에만 쓰고 저장하지 않아요. 터미널 Claude Code에 로그인된 계정의 값이 나와요. 기본은 꺼짐이에요.':
      'When on, DeskDusk asks Anthropic for your limits every 10 minutes using the Claude Code login stored on this computer. You may be asked to allow keychain access. The login is used only for that request and is never stored. Values are for the account signed in to terminal Claude Code. Off by default.',
    '켰어요. 곧 새 값으로 바뀌어요.': 'On. Values will update shortly.', '껐어요.': 'Off.',
    '로그인이 만료됐어요. Claude Code를 한 번 실행하면 다시 받아 와요.': 'Login expired. Run Claude Code once to refresh it.',
    '로그인이 만료됐거나 권한이 없어요. Claude Code를 한 번 실행해 주세요.': 'Login expired or not permitted. Run Claude Code once.',
    '서버가 잠시 요청을 막았어요. 30분 뒤 다시 물어요.': 'The server is rate limiting requests. Will retry in 30 minutes.',
    '서버에 연결하지 못했어요.': 'Could not reach the server.', '서버 응답 형식이 바뀐 것 같아요.': 'The server response format seems to have changed.',
    'Claude Code 로그인 정보를 찾지 못했어요. 터미널에서 Claude Code에 로그인해 주세요.': 'No Claude Code login found. Sign in to Claude Code in a terminal.',
    'Claude Code 로그인 정보를 읽지 못했어요.': 'Could not read the Claude Code login.',
    // 알림·메뉴
    '{0} {1} 한도가 {2}%를 넘었어요': '{0} {1} limit passed {2}%', '지금 {0} · 초기화까지 {1}': 'now {0} · resets in {1}',
    '대시보드 열기': 'Open dashboard', '종료': 'Quit',
    '모양': 'Shape', '풍경과 숫자': 'Scene and numbers', '숫자 띠': 'Number strip', '풍경만': 'Scene only',
    '보여 줄 항목': 'Show', '풍경 글자': 'Scene words', '한도 막대': 'Limit bars', '초기화까지 남은 시간': 'Time until reset',
    '오늘 토큰과 속도': 'Tokens today and rate', 'API 환산 비용': 'API-equivalent cost', '서비스': 'Services', '대시보드 따라가기': 'Follow dashboard',
    '위젯 하나 더 띄우기': 'Add another widget', '화면 가운데로 옮기기': 'Move to center of screen', '항상 위에 두기': 'Keep on top', '이 위젯 닫기': 'Close this widget',
    '위젯을 오른쪽 클릭하면 모양과 내용을 바꿀 수 있어요.': 'Right-click a widget to change its shape and contents.',
    '바탕화면 위젯': 'Desktop widget', '바탕화면 위젯 켜고 끄기': 'Toggle desktop widget', '위젯을 다른 창 위에 띄우기': 'Float widgets above windows',
  };

  // T('원문 {0}', 값…) → 언어에 맞는 글자
  // 윈도우에서는 맥 용어를 바꾼다: 메뉴바 → 트레이, 키체인 안내는 뺀다(윈도우는 로그인 정보 파일을 읽는다).
  // 주소에 ?plat=win 을 붙이면 다른 곳에서도 윈도우 문구를 볼 수 있다(점검용).
  const WIN = /Windows/.test(navigator.userAgent || '') || /[?&]plat=win\b/.test(location.search || '');
  window.IS_WIN = WIN;
  function plat(s) {
    if (!WIN || typeof s !== 'string') return s;
    return s
      .replace(' 켤 때 키체인 접근을 물어볼 수 있어요.', '')
      .replace(' You may be asked to allow keychain access.', '')
      .replace(/메뉴바/g, '트레이')
      .replace(/Menu bar/g, 'Tray')
      .replace(/menu bar/g, 'tray');
  }

  window.T = function (ko) {
    const args = arguments;
    const s = plat(lang === 'en' && Object.prototype.hasOwnProperty.call(EN, ko) ? EN[ko] : ko);
    return s.replace(/\{(\d)\}/g, (m, i) => (args[+i + 1] === undefined ? '' : String(args[+i + 1])));
  };

  // 정적 마크업의 글자와 aria-label을 바꾼다(영어일 때, 그리고 윈도우 용어로 바꿀 때).
  window.translateDom = function (root) {
    if (lang !== 'en' && !WIN) return;
    const en = lang === 'en';
    const walker = document.createTreeWalker(root || document.body, NodeFilter.SHOW_TEXT);
    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);
    for (const n of nodes) {
      const raw = n.nodeValue, key = raw.trim();
      if (!key) continue;
      const out = plat(en && Object.prototype.hasOwnProperty.call(EN, key) ? EN[key] : key);
      if (out !== key) n.nodeValue = raw.replace(key, out);
    }
    for (const attr of ['aria-label', 'title']) {
      (root || document).querySelectorAll('[' + attr + ']').forEach((el) => {
        const k = el.getAttribute(attr);
        const out = plat(en && Object.prototype.hasOwnProperty.call(EN, k) ? EN[k] : k);
        if (out !== k) el.setAttribute(attr, out);
      });
    }
    if (!en) return;
    document.documentElement.lang = 'en';
    if (Object.prototype.hasOwnProperty.call(EN, document.title)) document.title = EN[document.title];
  };
})();
