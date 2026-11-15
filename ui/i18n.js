'use strict';

window.FlyxI18n = (() => {
  const languages = [
    ['en','English','en-US'], ['ru','Русский','ru-RU'], ['zh','简体中文','zh-CN'],
    ['fa','فارسی','fa-IR'], ['ko','한국어','ko-KR'], ['tk','Türkmen','tk-TM'],
    ['uz','O‘zbek','uz-UZ'], ['az','Azərbaycan','az-AZ'], ['kk','Қазақша','kk-KZ'],
  ];
  // Each row: key | Russian | English | Chinese | Persian | Korean |
  // Turkmen | Uzbek | Azerbaijani | Kazakh. No remote translation service.
  const rows = `
home|Главная|Home|主页|خانه|홈|Baş sahypa|Bosh sahifa|Əsas|Басты
servers|Серверы|Servers|服务器|سرورها|서버|Serwerler|Serverlar|Serverlər|Серверлер
rules|Правила|Routing|路由|مسیریابی|라우팅|Düzgünler|Qoidalar|Qaydalar|Ережелер
settings|Настройки|Settings|设置|تنظیمات|설정|Sazlamalar|Sozlamalar|Tənzimləmələr|Баптаулар
idle|Отключено|Disconnected|未连接|اتصال قطع است|연결되지 않음|Birikdirilmedik|Ulanmagan|Qoşulmayıb|Қосылмаған
connected|Подключено|Connected|已连接|متصل است|연결됨|Birikdirildi|Ulangan|Qoşulub|Қосылған
failed|Не удалось|Connection failed|连接失败|اتصال ناموفق بود|연결 실패|Birikmek başartmady|Ulanib bo‘lmadi|Qoşulma alınmadı|Қосылу сәтсіз
connecting|Подключение…|Connecting…|正在连接…|در حال اتصال…|연결 중…|Birikdirilýär…|Ulanmoqda…|Qoşulur…|Қосылуда…
disconnecting|Отключение…|Disconnecting…|正在断开…|در حال قطع اتصال…|연결 해제 중…|Aýrylýar…|Uzilmoqda…|Ayrılır…|Ажыратылуда…
connect|Подключиться|Connect|连接|اتصال|연결|Birikdir|Ulanish|Qoşul|Қосылу
disconnect|Отключиться|Disconnect|断开|قطع اتصال|연결 해제|Aýyr|Uzish|Ayır|Ажырату
retry|Повторить|Try again|重试|تلاش مجدد|다시 시도|Gaýtala|Qayta urinish|Yenidən yoxla|Қайталау
wait|Подождите…|Please wait…|请稍候…|لطفاً صبر کنید…|잠시만 기다려 주세요…|Garaşyň…|Kuting…|Gözləyin…|Күтіңіз…
tapConnect|Нажмите, чтобы подключиться|Tap to connect|点击连接|برای اتصال کلیک کنید|눌러서 연결하세요|Birikmek üçin basyň|Ulanish uchun bosing|Qoşulmaq üçün basın|Қосылу үшін басыңыз
tapRetry|Нажмите, чтобы повторить|Tap to retry|点击重试|برای تلاش مجدد کلیک کنید|눌러서 다시 시도하세요|Gaýtalamak üçin basyň|Qayta urinish uchun bosing|Yenidən yoxlamaq üçün basın|Қайталау үшін басыңыз
moment|Секунду|Just a moment|请稍等|یک لحظه|잠시만요|Bir salym|Bir oz kuting|Bir an|Бір сәт
serviceMissing|Служба Flyx не запущена|Flyx service is not running|Flyx 服务未运行|سرویس Flyx اجرا نشده است|Flyx 서비스가 실행되지 않았습니다|Flyx hyzmaty işlemeýär|Flyx xizmati ishga tushmagan|Flyx xidməti işləmir|Flyx қызметі іске қосылмаған
noServer|Сервер не выбран|No server selected|未选择服务器|سروری انتخاب نشده|서버를 선택하지 않았습니다|Serwer saýlanmady|Server tanlanmagan|Server seçilməyib|Сервер таңдалмаған
addSource|Добавьте подписку или ключ|Add a subscription or key|添加订阅或密钥|اشتراک یا کلید اضافه کنید|구독 또는 키를 추가하세요|Abuna ýa-da açar goşuň|Obuna yoki kalit qo‘shing|Abunəlik və ya açar əlavə edin|Жазылым немесе кілт қосыңыз
allDevice|Весь компьютер|Whole device|整台设备|کل دستگاه|전체 기기|Tutuş kompýuter|Butun kompyuter|Bütün cihaz|Бүкіл компьютер
localProxy|Локальный прокси|Local proxy|本地代理|پروکسی محلی|로컬 프록시|Ýerli proksi|Mahalliy proksi|Yerli proksi|Жергілікті прокси
proxyPort|Прокси · порт {port}|Proxy · port {port}|代理 · 端口 {port}|پروکسی · درگاه {port}|프록시 · 포트 {port}|Proksi · port {port}|Proksi · port {port}|Proksi · port {port}|Прокси · порт {port}
apps|Приложения|Applications|应用|برنامه‌ها|앱|Programmalar|Ilovalar|Tətbiqlər|Қолданбалар
all|Все|All|全部|همه|모두|Ählisi|Barchasi|Hamısı|Барлығы
except|Кроме|Except|排除|به‌جز|제외|Başga|Istisno|İstisna|Басқасы
only|Только|Only|仅限|فقط|선택만|Diňe|Faqat|Yalnız|Тек
add|Добавить|Add|添加|افزودن|추가|Goş|Qo‘shish|Əlavə et|Қосу
directSites|Сайты напрямую|Direct access|直连网站|دسترسی مستقیم|직접 연결|Göni saýtlar|Bevosita saytlar|Birbaşa saytlar|Тікелей сайттар
ruSites|Российские сервисы|Russian services|俄罗斯服务|سرویس‌های روسیه|러시아 서비스|Russiýa hyzmatlary|Rossiya xizmatlari|Rusiya xidmətləri|Ресей сервистері
ruHint|.ru, .рф и выбранные сервисы — вне VPN|.ru, .рф and listed services bypass VPN|.ru、.рф 及所列服务不经过 VPN|دامنه‌های ‎.ru، ‎.рф و سرویس‌های فهرست‌شده خارج از VPN|.ru, .рф 및 목록의 서비스는 VPN을 우회합니다|.ru, .рф we sanawdaky hyzmatlar VPN-den daşarda|.ru, .рф va ro‘yxatdagi xizmatlar VPN tashqarisida|.ru, .рф və siyahıdakı xidmətlər VPN-dən kənardır|.ru, .рф және тізімдегі сервистер VPN-нен тыс
scope|Что защищать|Traffic protection|保护范围|محدودهٔ حفاظت|보호 범위|Gorag çägi|Himoya doirasi|Qoruma sahəsi|Қорғау ауқымы
tunHint|Через VPN идёт трафик всех программ, включая игры.|Routes application and game traffic through VPN.|所有应用和游戏流量均通过 VPN。|ترافیک برنامه‌ها و بازی‌ها از VPN عبور می‌کند.|앱과 게임의 트래픽을 VPN으로 전송합니다.|Programmalaryň we oýunlaryň traffigi VPN arkaly geçýär.|Ilovalar va o‘yinlar trafigi VPN orqali o‘tadi.|Tətbiq və oyun trafiki VPN-dən keçir.|Қолданбалар мен ойындардың трафигі VPN арқылы өтеді.
proxyHint|Укажите прокси 127.0.0.1:{port} в нужной программе.|Set proxy 127.0.0.1:{port} in your application.|请在应用中设置代理 127.0.0.1:{port}。|پروکسی 127.0.0.1:{port} را در برنامه تنظیم کنید.|앱에서 프록시 127.0.0.1:{port}를 설정하세요.|Programmada 127.0.0.1:{port} proksisini belläň.|Ilovada 127.0.0.1:{port} proksisini sozlang.|Tətbiqdə 127.0.0.1:{port} proksisini qurun.|Қолданбада 127.0.0.1:{port} проксиін орнатыңыз.
splitAll|Все программы работают через VPN.|All applications use VPN.|所有应用均使用 VPN。|همهٔ برنامه‌ها از VPN استفاده می‌کنند.|모든 앱이 VPN을 사용합니다.|Ähli programmalar VPN ulanýar.|Barcha ilovalar VPN orqali ishlaydi.|Bütün tətbiqlər VPN istifadə edir.|Барлық қолданбалар VPN арқылы жұмыс істейді.
splitExcept|Выбранные программы — напрямую, остальные — через VPN.|Selected applications connect directly; others use VPN.|所选应用直连，其他应用使用 VPN。|برنامه‌های انتخاب‌شده مستقیم و بقیه از طریق VPN متصل می‌شوند.|선택한 앱은 직접 연결하고 나머지는 VPN을 사용합니다.|Saýlananlar göni, galan programmalar VPN arkaly işleýär.|Tanlangan ilovalar bevosita, qolganlari VPN orqali ishlaydi.|Seçilmiş tətbiqlər birbaşa, digərləri VPN ilə işləyir.|Таңдалған қолданбалар тікелей, қалғандары VPN арқылы жұмыс істейді.
splitOnly|Через VPN работают только выбранные программы.|Only selected applications use VPN.|仅所选应用使用 VPN。|فقط برنامه‌های انتخاب‌شده از VPN استفاده می‌کنند.|선택한 앱만 VPN을 사용합니다.|Diňe saýlanan programmalar VPN ulanýar.|Faqat tanlangan ilovalar VPN orqali ishlaydi.|Yalnız seçilmiş tətbiqlər VPN istifadə edir.|Тек таңдалған қолданбалар VPN арқылы жұмыс істейді.
apply|Применить изменения|Apply changes|应用更改|اعمال تغییرات|변경 적용|Üýtgetmeleri ulan|O‘zgarishlarni qo‘llash|Dəyişiklikləri tətbiq et|Өзгерістерді қолдану
logs|Журнал|Logs|日志|گزارش‌ها|로그|Žurnal|Jurnal|Jurnal|Журнал
logTitle|Журнал подключения|Connection log|连接日志|گزارش اتصال|연결 로그|Birikme žurnaly|Ulanish jurnali|Bağlantı jurnalı|Қосылым журналы
close|Закрыть|Close|关闭|بستن|닫기|Ýap|Yopish|Bağla|Жабу
minimize|Свернуть|Minimize|最小化|کوچک کردن|최소화|Kiçelt|Kichraytirish|Kiçilt|Кішірейту
hideTray|Скрыть в трей|Hide to tray|隐藏到托盘|پنهان‌کردن در سینی سیستم|트레이로 숨기기|Treýe gizle|Treyga yashirish|Treyə gizlət|Трейге жасыру
hideHint|Скрыть в трей — VPN продолжит работать|Hide to tray — VPN stays connected|隐藏到托盘，VPN 保持连接|پنهان‌کردن در سینی؛ VPN فعال می‌ماند|트레이로 숨겨도 VPN은 유지됩니다|Treýe gizlense-de VPN işlemegini dowam edýär|Treyga yashirilganda VPN ishlashda davom etadi|Treyə gizlədildikdə VPN işləməyə davam edir|Трейге жасырғанда VPN жұмысын жалғастырады
powerLabel|Подключить или отключить VPN|Connect or disconnect VPN|连接或断开 VPN|اتصال یا قطع VPN|VPN 연결 또는 해제|VPN-i birikdir ýa-da aýyr|VPNni ulash yoki uzish|VPN-i qoş və ya ayır|VPN қосу немесе ажырату
sourcePlaceholder|Ссылка на подписку или ключ|Subscription link or key|订阅链接或密钥|پیوند اشتراک یا کلید|구독 링크 또는 키|Abuna salgysy ýa-da açar|Obuna havolasi yoki kalit|Abunəlik linki və ya açar|Жазылым сілтемесі немесе кілт
theme|Оформление|Appearance|外观|ظاهر|화면 설정|Daşky görnüş|Ko‘rinish|Görünüş|Безендіру
themeHint|Изменения оформления сохраняются автоматически.|Appearance changes are saved automatically.|外观更改会自动保存。|تغییرات ظاهر خودکار ذخیره می‌شوند.|화면 설정은 자동으로 저장됩니다.|Daşky görnüş awtomatik ýatda saklanýar.|Ko‘rinish o‘zgarishlari avtomatik saqlanadi.|Görünüş dəyişiklikləri avtomatik saxlanılır.|Безендіру өзгерістері автоматты сақталады.
themeLabel|Тема приложения|Application theme|应用主题|پوستهٔ برنامه|앱 테마|Programma temasy|Ilova mavzusi|Tətbiq mövzusu|Қолданба тақырыбы
light|Светлая|Light|浅色|روشن|라이트|Ýagty|Yorug‘|Açıq|Ашық
dark|Тёмная|Dark|深色|تیره|다크|Garaňky|Qorong‘i|Tünd|Қараңғы
language|Язык|Language|语言|زبان|언어|Dil|Til|Dil|Тіл
startup|При запуске|On startup|启动时|هنگام اجرا|시작 시|Işe başlanda|Ishga tushganda|Başlanğıcda|Іске қосылғанда
autoConnect|Автоподключение|Auto-connect|自动连接|اتصال خودکار|자동 연결|Awtomatik birikme|Avtomatik ulanish|Avtomatik qoşulma|Автоматты қосылу
autoConnectHint|При запуске подключаться к последнему выбранному серверу|Connect to the last selected server on startup|启动时连接到上次选择的服务器|هنگام اجرا به آخرین سرور انتخاب‌شده متصل شود|시작 시 마지막으로 선택한 서버에 연결합니다|Işe başlanda soňky saýlanan serwere birik|Ishga tushganda oxirgi tanlangan serverga ulanish|Başladıqda son seçilmiş serverə qoşul|Іске қосылғанда соңғы таңдалған серверге қосылу
startHidden|Запуск в трее|Start in tray|启动到托盘|شروع در سینی سیستم|트레이에서 시작|Treýde başlat|Treyda ishga tushirish|Treydə başlat|Трейде іске қосу
startHiddenHint|Скрывать окно при следующем запуске|Hide the window on the next launch|下次启动时隐藏窗口|در اجرای بعدی پنجره پنهان شود|다음 실행 시 창을 숨깁니다|Indiki açylyşda penjiräni gizle|Keyingi ishga tushishda oynani yashirish|Növbəti açılışda pəncərəni gizlət|Келесі іске қосқанда терезені жасыру
diagnostics|Помощь|Help|帮助|راهنما|도움말|Kömek|Yordam|Kömək|Көмек
about|Flyx · VPN-клиент для {platform}|Flyx · VPN client for {platform}|Flyx · {platform} VPN 客户端|Flyx · برنامهٔ VPN برای {platform}|Flyx · {platform}용 VPN 클라이언트|Flyx · {platform} üçin VPN müşderisi|Flyx · {platform} uchun VPN mijoz|Flyx · {platform} üçün VPN müştərisi|Flyx · {platform} үшін VPN клиенті
refresh|Обновить подписку|Refresh subscription|更新订阅|به‌روزرسانی اشتراک|구독 새로고침|Abunany täzele|Obunani yangilash|Abunəliyi yenilə|Жазылымды жаңарту
logAll|Все|All|全部|همه|전체|Ählisi|Hammasi|Hamısı|Барлығы
logErrors|Ошибки|Errors|错误|خطاها|오류|Ýalňyşlyklar|Xatolar|Xətalar|Қателер
logCopy|Копировать|Copy|复制|کپی|복사|Göçür|Nusxalash|Kopyala|Көшіру
logCopied|Скопировано|Copied|已复制|کپی شد|복사됨|Göçürildi|Nusxalandi|Kopyalandı|Көшірілді
pingAll|Проверить пинг всех серверов|Ping all servers|测试所有服务器延迟|پینگ همه سرورها|모든 서버 핑 측정|Ähli serwerleriň pingini barla|Barcha serverlar pingini tekshirish|Bütün serverlərin pinqini yoxla|Барлық серверлердің пингін тексеру
pingTun|Отключите VPN, чтобы измерить пинг: в режиме «Весь компьютер» замер идёт через туннель.|Disconnect the VPN to measure ping: in whole-device mode the test runs through the tunnel.|请先断开 VPN 再测试延迟：全局模式下测试会经过隧道。|برای اندازه‌گیری پینگ VPN را قطع کنید: در حالت کل دستگاه، آزمون از داخل تونل انجام می‌شود.|핑을 측정하려면 VPN 연결을 해제하세요. 전체 기기 모드에서는 측정이 터널을 통해 이루어집니다.|Pingi ölçemek üçin VPN-i aýryň: tutuş enjam režiminde barlag tuneliň üsti bilen geçýär.|Pingni o‘lchash uchun VPNni uzing: butun qurilma rejimida tekshiruv tunnel orqali o‘tadi.|Pinqi ölçmək üçün VPN-i ayırın: bütün cihaz rejimində yoxlama tunel vasitəsilə gedir.|Пингті өлшеу үшін VPN-ді ажыратыңыз: бүкіл құрылғы режимінде тексеру туннель арқылы өтеді.
pingDone|Пинг измерен: ответили {ok} из {total}.|Ping measured: {ok} of {total} responded.|延迟测试完成：{total} 个中有 {ok} 个响应。|پینگ اندازه‌گیری شد: {ok} از {total} پاسخ دادند.|핑 측정 완료: {total}개 중 {ok}개 응답.|Ping ölçeldi: {total} serwerden {ok} sanysy jogap berdi.|Ping o‘lchandi: {total} tadan {ok} tasi javob berdi.|Pinq ölçüldü: {total} serverdən {ok}-i cavab verdi.|Пинг өлшенді: {total} серверден {ok} жауап берді.
delete|Удалить|Delete|删除|حذف|삭제|Poz|O‘chirish|Sil|Жою
deleteSubscription|Удалить подписку|Delete subscription|删除订阅|حذف اشتراک|구독 삭제|Abunany poz|Obunani o‘chirish|Abunəliyi sil|Жазылымды жою
emptyServers|Добавьте подписку или ключ — серверы появятся здесь.|Add a subscription or key to see your servers here.|添加订阅或密钥后，服务器将显示在这里。|اشتراک یا کلید اضافه کنید تا سرورها اینجا نمایش داده شوند.|구독 또는 키를 추가하면 서버가 여기에 표시됩니다.|Abuna ýa-da açar goşuň, serwerler şu ýerde peýda bolar.|Obuna yoki kalit qo‘shing — serverlar shu yerda ko‘rinadi.|Abunəlik və ya açar əlavə edin — serverlər burada görünəcək.|Жазылым немесе кілт қоссаңыз, серверлер осында пайда болады.
refreshEmpty|Обновите подписку для получения совместимых серверов.|Refresh this subscription to load compatible servers.|请更新订阅以获取兼容的服务器。|برای دریافت سرورهای سازگار، اشتراک را به‌روز کنید.|호환되는 서버를 가져오려면 구독을 새로고침하세요.|Gabat gelýän serwerler üçin abunany täzeläň.|Mos serverlarni olish uchun obunani yangilang.|Uyğun serverləri almaq üçün abunəliyi yeniləyin.|Үйлесімді серверлерді алу үшін жазылымды жаңартыңыз.
myKeys|Мои ключи|My keys|我的密钥|کلیدهای من|내 키|Açarlarym|Kalitlarim|Açarlarım|Менің кілттерім
added|Добавлено: {count}.|Added: {count}.|已添加：{count}。|افزوده شد: {count}.|추가됨: {count}.|Goşuldy: {count}.|Qo‘shildi: {count}.|Əlavə edildi: {count}.|Қосылды: {count}.
skipped|Пропущено: {n}. {reason}|Skipped: {n}. {reason}|已跳过：{n}。{reason}|رد شد: {n}. {reason}|건너뜀: {n}. {reason}|Geçirildi: {n}. {reason}|O‘tkazib yuborildi: {n}. {reason}|Buraxıldı: {n}. {reason}|Өткізілді: {n}. {reason}
updated|«{name}» обновлена.|“{name}” updated.|“{name}”已更新。|«{name}» به‌روز شد.|“{name}” 업데이트됨.|«{name}» täzelendi.|«{name}» yangilandi.|«{name}» yeniləndi.|«{name}» жаңартылды.
until|до {date}|until {date}|有效期至 {date}|تا {date}|{date}까지|{date} çenli|{date} gacha|{date} tarixinədək|{date} дейін
usage|{used} из {total}|{used} of {total}|{used} / {total}|{used} از {total}|{used} / {total}|{total}-dan {used}|{total} dan {used}|{total}-dan {used}|{total} ішінен {used}
gb|ГБ|GB|GB|گیگابایت|GB|GB|GB|GB|ГБ
ms|мс|ms|毫秒|میلی‌ثانیه|ms|ms|ms|ms|мс
loading|Загружаю…|Loading…|正在加载…|در حال بارگذاری…|불러오는 중…|Ýüklenýär…|Yuklanmoqda…|Yüklənir…|Жүктелуде…
noLogs|Журнал пуст.|No log entries.|暂无日志。|گزارش خالی است.|로그가 없습니다.|Žurnal boş.|Jurnal bo‘sh.|Jurnal boşdur.|Журнал бос.
removeItem|Убрать {item}|Remove {item}|移除 {item}|حذف {item}|{item} 삭제|{item} aýyr|{item}ni olib tashlash|{item} sil|{item} жою
viaXray|через Xray|via Xray|通过 Xray|از طریق Xray|Xray 경유|Xray arkaly|Xray orqali|Xray vasitəsilə|Xray арқылы
allApps|Все приложения|All applications|所有应用|همهٔ برنامه‌ها|모든 앱|Ähli programmalar|Barcha ilovalar|Bütün tətbiqlər|Барлық қолданбалар
onlyApps|Только выбранные · {n}|Selected only · {n}|仅所选 · {n}|فقط انتخاب‌شده‌ها · {n}|선택한 앱만 · {n}|Diňe saýlananlar · {n}|Faqat tanlanganlar · {n}|Yalnız seçilənlər · {n}|Тек таңдалғандар · {n}
exceptApps|Кроме выбранных · {n}|Except selected · {n}|排除所选 · {n}|به‌جز انتخاب‌شده‌ها · {n}|선택한 앱 제외 · {n}|Saýlananlardan başga · {n}|Tanlanganlardan tashqari · {n}|Seçilənlərdən başqa · {n}|Таңдалғандардан басқа · {n}
requestFailed|Не удалось выполнить запрос.|The request could not be completed.|无法完成请求。|درخواست انجام نشد.|요청을 완료하지 못했습니다.|Talaby ýerine ýetirmek başartmady.|So‘rov bajarilmadi.|Sorğunu tamamlamaq mümkün olmadı.|Сұрауды орындау мүмкін болмады.
noNodes|Серверы не найдены. Обновите подписку.|No servers found. Refresh the subscription.|未找到服务器，请更新订阅。|سروری یافت نشد؛ اشتراک را به‌روز کنید.|서버를 찾지 못했습니다. 구독을 새로고침하세요.|Serwer tapylmady. Abunany täzeläň.|Server topilmadi. Obunani yangilang.|Server tapılmadı. Abunəliyi yeniləyin.|Сервер табылмады. Жазылымды жаңартыңыз.
providerDenied|Провайдер не выдал совместимую конфигурацию.|The provider did not return a compatible configuration.|服务商未返回兼容的配置。|ارائه‌دهنده پیکربندی سازگار نداد.|제공업체가 호환되는 설정을 반환하지 않았습니다.|Üpjün ediji gabat gelýän sazlama bermedi.|Provayder mos konfiguratsiya bermadi.|Provayder uyğun konfiqurasiya qaytarmadı.|Провайдер үйлесімді конфигурация бермеді.
fetchFailed|Не удалось скачать подписку.|Could not download the subscription.|无法下载订阅。|دریافت اشتراک ممکن نشد.|구독을 다운로드하지 못했습니다.|Abunany ýüklemek başartmady.|Obunani yuklab bo‘lmadi.|Abunəliyi yükləmək mümkün olmadı.|Жазылымды жүктеу мүмкін болмады.
ad|Реклама|Advertisement|广告|تبلیغات|광고|Mahabat|Reklama|Reklam|Жарнама
adOpen|Открыть сайт рекламодателя|Open advertiser website|打开广告网站|بازکردن سایت تبلیغ‌دهنده|광고주 사이트 열기|Mahabatçynyň saýtyny aç|Reklama beruvchi saytini ochish|Reklamverənin saytını aç|Жарнама берушінің сайтын ашу
serviceList|Список сервисов|Service list|服务列表|فهرست سرویس‌ها|서비스 목록|Hyzmatlaryň sanawy|Xizmatlar ro‘yxati|Xidmət siyahısı|Сервистер тізімі
shopping|Покупки|Shopping|购物|خرید|쇼핑|Söwda|Xaridlar|Alış-veriş|Сауда
banks|Банки и платежи|Banking and payments|银行与支付|بانک و پرداخت|은행 및 결제|Banklar we tölegler|Banklar va to‘lovlar|Banklar və ödənişlər|Банктер және төлемдер
government|Госуслуги|Government services|政务服务|خدمات دولتی|공공 서비스|Döwlet hyzmatlary|Davlat xizmatlari|Dövlət xidmətləri|Мемлекеттік қызметтер
delivery|Еда и доставка|Food and delivery|餐饮与配送|غذا و تحویل|음식 및 배달|Iýmit we eltip bermek|Oziq-ovqat va yetkazish|Yemək və çatdırılma|Тамақ және жеткізу
media|Медиа и связь|Media and communication|媒体与通讯|رسانه و ارتباطات|미디어 및 소통|Media we aragatnaşyk|Media va aloqa|Media və əlaqə|Медиа және байланыс
telecom|Операторы|Telecom|电信运营商|اپراتورها|통신사|Operatorlar|Operatorlar|Operatorlar|Операторлар
travel|Поездки|Travel|出行|سفر|여행|Syýahat|Sayohat|Səyahət|Сапарлар
work|Работа и недвижимость|Jobs and housing|工作与住房|کار و مسکن|취업 및 주거|Iş we ýaşaýyş|Ish va uy-joy|İş və yaşayış|Жұмыс және тұрғын үй
updates|Обновления|Updates|更新|به‌روزرسانی‌ها|업데이트|Täzelenmeler|Yangilanishlar|Yeniləmələr|Жаңартулар
updateAvailable|Доступно обновление|Update available|有可用更新|به‌روزرسانی موجود است|업데이트 가능|Täzelenme bar|Yangilanish mavjud|Yeniləmə mövcuddur|Жаңарту қолжетімді
upToDate|Установлена последняя версия|You’re up to date|已是最新版本|نسخهٔ شما به‌روز است|최신 버전입니다|Iň täze wersiýa|Eng so‘nggi versiya|Ən son versiyadır|Соңғы нұсқа орнатылған
latestHint|Новые версии появляются здесь автоматически.|New releases are checked automatically.|将自动检查新版本。|نسخه‌های جدید خودکار بررسی می‌شوند.|새 버전을 자동으로 확인합니다.|Täze wersiýalar awtomatik barlanýar.|Yangi versiyalar avtomatik tekshiriladi.|Yeni versiyalar avtomatik yoxlanılır.|Жаңа нұсқалар автоматты тексеріледі.
updateHint|Установка перезапустит Flyx и временно отключит VPN. При необходимости система запросит разрешение.|Installation restarts Flyx and temporarily disconnects VPN. Your system may ask for permission.|安装将重启 Flyx 并暂时断开 VPN，系统可能会请求授权。|نصب، Flyx را دوباره اجرا و VPN را موقتاً قطع می‌کند. ممکن است سیستم اجازه بخواهد.|설치 시 Flyx가 재시작되고 VPN이 잠시 끊깁니다. 시스템에서 권한을 요청할 수 있습니다.|Gurnama Flyx-y täzeden açar we VPN-i wagtlaýyn keser. Ulgam rugsat sorap biler.|O‘rnatish Flyx’ni qayta ishga tushiradi va VPN’ni vaqtincha uzadi. Tizim ruxsat so‘rashi mumkin.|Quraşdırma Flyx-i yenidən başladır və VPN-i müvəqqəti ayırır. Sistem icazə istəyə bilər.|Орнату Flyx қолданбасын қайта іске қосып, VPN-ді уақытша ажыратады. Жүйе рұқсат сұрауы мүмкін.
updateNow|Обновить|Update now|立即更新|به‌روزرسانی|지금 업데이트|Täzele|Yangilash|Yenilə|Жаңарту
later|Позже|Later|稍后|بعداً|나중에|Soňrak|Keyinroq|Sonra|Кейін
updateError|Не удалось обновить Flyx|Couldn’t update Flyx|无法更新 Flyx|به‌روزرسانی Flyx انجام نشد|Flyx 업데이트 실패|Flyx täzelenmedi|Flyx yangilanmadi|Flyx yenilənmədi|Flyx жаңартылмады
updateRetry|Проверьте интернет и попробуйте снова через раздел «Обновления».|Check your connection and try again from Updates.|请检查网络后在更新中重试。|اتصال را بررسی و از بخش به‌روزرسانی دوباره تلاش کنید.|인터넷을 확인하고 업데이트에서 다시 시도하세요.|Interneti barlaň we Täzelenmelerden gaýtadan synanyşyň.|Internetni tekshiring va Yangilanishlar bo‘limida qayta urinib ko‘ring.|İnterneti yoxlayın və Yeniləmələr bölməsində yenidən cəhd edin.|Интернетті тексеріп, Жаңартулар бөлімінен қайталаңыз.
downloading|Загружаем и проверяем подпись…|Downloading and verifying signature…|正在下载并验证签名…|در حال دریافت و بررسی امضا…|다운로드 및 서명 확인 중…|Ýüklenýär we gol barlanýar…|Yuklanmoqda va imzo tekshirilmoqda…|Endirilir və imza yoxlanılır…|Жүктеліп, қолтаңба тексерілуде…
installing|Устанавливаем обновление…|Installing update…|正在安装更新…|در حال نصب به‌روزرسانی…|업데이트 설치 중…|Täzelenme gurnalýar…|Yangilanish o‘rnatilmoqda…|Yeniləmə quraşdırılır…|Жаңарту орнатылуда…
`;
  const order=['ru','en','zh','fa','ko','tk','uz','az','kk'];
  const dictionaries=Object.fromEntries(order.map(code=>[code,{}]));
  for(const row of rows.trim().split('\n')) {
    const [key,...values]=row.split('|');
    if(values.length!==9) throw Error(`Translation columns: ${key}`);
    order.forEach((code,i)=>{dictionaries[code][key]=values[i];});
  }
  let code='ru', platform='Windows';
  function t(key, values={}) {
    const value=dictionaries[code][key] ?? dictionaries.en[key] ?? key;
    return value.replace(/\{(\w+)\}/g,(_,name)=>String(values[name]??(name==='platform'?platform:`{${name}}`)));
  }
  function apply(root=document) {
    root.querySelectorAll('[data-i18n]').forEach(node=>node.textContent=t(node.dataset.i18n));
    for(const attr of ['placeholder','title','aria-label']) {
      root.querySelectorAll(`[data-i18n-${attr}]`).forEach(node=>node.setAttribute(attr,t(node.getAttribute(`data-i18n-${attr}`))));
    }
  }
  const words={ru:['сервер','сервера','серверов','сервера'],en:['server','servers','servers','servers'],zh:['个服务器'],fa:['سرور'],ko:['개 서버'],tk:['serwer'],uz:['server'],az:['server'],kk:['сервер']};
  function countServers(n) {
    const plural=new Intl.PluralRules(locale()).select(n);
    const list=words[code],index={one:0,few:1,many:2,other:3}[plural]??3;
    return `${new Intl.NumberFormat(locale()).format(n)} ${list[index]??list[0]}`;
  }
  function locale(){return languages.find(l=>l[0]===code)[2];}
  function error(raw) {
    const message=String(raw);
    if(code==='ru') return message;
    if(/служба.+не запущена|служба.+не отвечает/i.test(message))return t('serviceMissing');
    if(/серверов не найдено/i.test(message))return t('noNodes');
    if(/провайдер не выдал|провайдер не вернул|служебное сообщение/i.test(message))return t('providerDenied');
    if(/не удалось скачать/i.test(message))return t('fetchFailed');
    return `${t('requestFailed')} ${message}`;
  }
  return {t,apply,languages,locale,countServers,error,dictionaries,
    setPlatform(value){platform=['Linux','Android'].includes(value)?value:'Windows';apply();},
    set(next){code=order.includes(next)?next:'ru';document.documentElement.lang=code;document.documentElement.dir=code==='fa'?'rtl':'ltr';apply();return code;},
    get code(){return code;},
  };
})();
