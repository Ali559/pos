// Shop words → English keywords that appear in Tabler icon names or tags.
// Each entry: [English keywords, words that should suggest them]. The words
// are Kurdish (Sorani), Arabic and common English shop terms; they're
// matched after normalizing letter variants (ي/ی, ك/ک, alef forms), and as
// prefixes, so suffixed forms (شیرەکە, الحليب) still match.
// Grow this list as shops name things the suggester misses.

export const ICON_SYNONYMS: [string, string][] = [
  // Dairy & eggs
  ["milk dairy", "شیر|شیرەمەنی|حلیب|البان|لبن|dairy"],
  ["milk dairy", "ماست|زبادی|روب|yogurt|yoghurt|laban|ayran|دۆ|دوغ"],
  ["cheese dairy", "پەنیر|جبن|جبنە|cheese"],
  ["egg", "هێلکە|بیض|بیضە|eggs"],
  ["cheese dairy", "کەرە|زبدە|قشطە|قەیماغ|butter|cream"],

  // Bakery & sweets
  ["bread bakery", "نان|سەمون|صمون|خبز|مخبز|نانەوایی|مخبوزات|samoon|bakery|bread|toast"],
  ["baguette bread", "باگێت|باکیت|baguette"],
  ["cookie", "بسکیت|بسکویت|کلێچە|کلیجە|کعک|klicha|biscuit|cookies"],
  ["cake", "کێک|کیک|کیکە|تورتە|cake"],
  ["candy lollipop", "شیرینی|حلویات|حلوی|سکاکر|sweets|candy"],
  ["chocolate", "شوکولاتە|شکولاتە|چۆکلێت|کاکاو|chocolate|cocoa"],
  ["ice-cream", "دۆندرمە|ئایسکرێم|بوظە|ایس کریم|ice cream|icecream"],

  // Drinks
  ["bottle drink", "خواردنەوە|مشروبات|مشروب|بوتڵ|قنینە|drink|drinks|beverage|beverages"],
  ["bottle", "کۆلا|کولا|پێپسی|بیبسی|ببسی|سەڤن|cola|pepsi|soda|soft|غازی|غازیە|گازی"],
  ["bottle droplet", "ئاو|ماء|میاە|water"],
  ["bottle", "شەربەت|عصیر|ئاوی میوە|juice"],
  ["coffee", "قاوە|قهوە|نسکافە|نسکافیە|nescafe|coffee|espresso"],
  ["cup coffee", "چا|چای|شای|tea"],
  ["beer", "بیرە|beer"],
  ["glass", "گڵاس|کاس|کوب|glass"],

  // Produce
  ["apple fruit", "سێو|تفاح|میوە|فواکە|فاکهە|fruit|fruits|apple"],
  ["lemon", "لیمۆ|لیمون|نارنج|lemon|orange"],
  ["cherry", "گێلاس|کرز|cherry"],
  ["carrot", "گێزەر|جزر|سەوزە|خضار|خضراوات|خضروات|vegetable|vegetables|produce|carrot"],
  ["salad", "زەڵاتە|سلطە|خەیار|خیار|تەماتە|طماطم|طماطە|cucumber|tomato|tomatoes|salad"],
  ["plant", "پیاز|بصل|سیر|ثوم|onion|garlic|herbs|کەوەر"],

  // Meat & meals
  ["meat", "گۆشت|لحم|لحوم|قیمە|کەباب|کباب|meat|beef|lamb"],
  ["sausage", "سۆسیج|سجق|نقانق|سوجوق|sausage|hotdog"],
  ["meat", "مریشک|دجاج|chicken"],
  ["fish", "ماسی|سمک|اسماک|fish|tuna|تونە"],
  ["pizza", "پیتزا|بیتزا|pizza"],
  ["burger", "بەرگەر|برغر|برجر|burger|sandwich|ساندویچ"],
  ["soup", "شۆربا|شوربە|soup"],

  // Pantry
  ["wheat", "گەنم|قمح|برنج|رز|ارز|ئارد|طحین|flour|rice|grain|دانەوێڵە|حبوب|نیسک|عدس|lentils|beans"],
  ["droplet", "ڕۆن|زیت|دهن|oil"],
  ["salt", "خوێ|ملح|salt"],
  ["pepper", "بیبەر|فلفل|بەهارات|بهارات|توابل|spices|spice|pepper"],
  ["cube", "شەکر|سکر|sugar"],
  ["box", "قوتو|معلبات|علبە|canned|can|tin"],
  ["snack cookie", "سووکەخۆراک|وجبات خفیفە|سناک|چیپس|شیبس|chips|crisps|snack|snacks"],
  ["acorn nut", "نۆک|پستە|فستق|مکسرات|گوێز|جوز|nuts|pistachio|seeds"],
  ["snowflake", "بەستوو|مجمد|مجمدات|frozen|سارد|بارد"],
  ["flame", "گەرم|ساخن|hot"],

  // Household & personal care
  ["spray", "پاککەرەوە|منظف|منظفات|تنظیف|clean|cleaning|detergent|بلیچ|chlorine"],
  ["bath droplet", "شامپۆ|شامبو|صابون|سابون|soap|shampoo|wash"],
  ["dental", "ددان|معجون|اسنان|فرشاە|toothpaste|toothbrush"],
  ["baby-carriage", "منداڵ|طفل|اطفال|پەمپەرز|حفاظات|baby|diaper|diapers"],
  ["perfume", "بۆن|عطر|عطور|جوانکاری|تجمیل|مکیاج|makeup|perfume|cosmetics"],
  ["paper", "کاغەز|ورق|کلینکس|مندیل|منادیل|tissue|tissues|paper"],
  ["shopping-bag", "کیسە|کیس|اکیاس|bag|bags|nylon"],
  ["tools-kitchen", "چێشتخانە|مطبخ|kitchen|cutlery|plates|قاپ|صحون"],
  ["home", "ماڵ|منزل|منزلیە|home|household"],

  // Health
  ["pill", "دەرمان|دواء|ادویە|صیدلیە|pharmacy|medicine|pills"],
  ["smoking", "جگەرە|سیجارە|سکایر|دخان|تتن|tobacco|cigarette|cigarettes"],

  // Electronics & other goods
  ["device-mobile", "مۆبایل|موبایل|هاتف|کارت|رصید|mobile|phone|recharge"],
  ["battery", "پاتری|بطاریە|battery|batteries"],
  ["bulb", "گڵۆپ|مصباح|لمبە|bulb|light"],
  ["device", "ئەلیکترۆنی|الکترونیات|electronics"],
  ["tool", "ئامراز|عدە|ادوات|tools|hardware"],
  ["shirt", "جل|جلوبەرگ|ملابس|قمیص|clothes|clothing|shirt"],
  ["shoe", "پێڵاو|حذاء|احذیە|shoes|shoe"],
  ["book pencil", "کتێب|کتاب|قرطاسیە|پێنووس|قلم|pen|stationery|book"],
  ["gift", "دیاری|هدیە|هدایا|gift|gifts"],
  ["horse-toy", "یاری|العاب|لعبە|toys|toy"],
  ["box", "کاڵا|منتجات|products|goods|other"],
];
