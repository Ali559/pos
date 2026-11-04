// Usual categories by shop type, offered as a checklist during setup.
// Names are [English, Kurdish (Sorani), Arabic]; only the chosen language
// is saved. Icons are Tabler keys (validated against the icon index).
import type { ShopType } from "@/db/shop";

type Names = [string, string, string];

export interface StarterTemplate {
  key: string;
  names: Names;
  icon: string;
  color?: string;
  /** Supermarket only */
  children?: { key: string; names: Names; icon: string }[];
}

const COLORS = ["#2d5a8f", "#94600c", "#5f4996", "#9a3f26", "#2f6b38", "#0e7490", "#be185d", "#57534e"];

const ALL: StarterTemplate[] = [
  {
    key: "beverages", names: ["Beverages", "خواردنەوە", "مشروبات"], icon: "tabler:bottle",
    children: [
      { key: "water", names: ["Water", "ئاو", "مياه"], icon: "tabler:droplet" },
      { key: "soft", names: ["Soft drinks", "خواردنەوەی گازی", "مشروبات غازية"], icon: "tabler:bottle" },
      { key: "juice", names: ["Juice", "شەربەت", "عصائر"], icon: "tabler:lemon" },
      { key: "energy", names: ["Energy drinks", "خواردنەوەی وزە", "مشروبات الطاقة"], icon: "tabler:bolt" },
    ],
  },
  {
    key: "snacks", names: ["Snacks & sweets", "سووکەخۆراک و شیرینی", "وجبات خفيفة وحلويات"], icon: "tabler:candy",
    children: [
      { key: "chips", names: ["Chips", "چیپس", "شيبس"], icon: "tabler:cookie" },
      { key: "chocolate", names: ["Chocolate", "شوکولاتە", "شوكولاتة"], icon: "tabler:chocolate" },
      { key: "biscuits", names: ["Biscuits", "بسکیت", "بسكويت"], icon: "tabler:cookie" },
      { key: "nuts", names: ["Nuts & seeds", "چەرەز", "مكسرات"], icon: "tabler:acorn" },
    ],
  },
  {
    key: "dairy", names: ["Dairy & eggs", "شیرەمەنی و هێلکە", "ألبان وبيض"], icon: "tabler:milk",
    children: [
      { key: "milk", names: ["Milk", "شیر", "حليب"], icon: "tabler:milk" },
      { key: "yogurt", names: ["Yogurt", "ماست", "لبن"], icon: "tabler:milk" },
      { key: "cheese", names: ["Cheese", "پەنیر", "أجبان"], icon: "tabler:cheese" },
      { key: "eggs", names: ["Eggs", "هێلکە", "بيض"], icon: "tabler:egg" },
    ],
  },
  {
    key: "bakery", names: ["Bread & bakery", "نان و نانەوایی", "خبز ومخبوزات"], icon: "tabler:baguette",
    children: [
      { key: "bread", names: ["Bread", "نان", "خبز"], icon: "tabler:baguette" },
      { key: "cakes", names: ["Cakes & sweets", "کێک و شیرینی", "كيك وحلويات"], icon: "tabler:cake" },
    ],
  },
  {
    key: "produce", names: ["Fruit & vegetables", "میوە و سەوزە", "فواكه وخضار"], icon: "tabler:apple",
    children: [
      { key: "fruit", names: ["Fruit", "میوە", "فواكه"], icon: "tabler:apple" },
      { key: "vegetables", names: ["Vegetables", "سەوزە", "خضار"], icon: "tabler:carrot" },
    ],
  },
  {
    key: "meat", names: ["Meat & poultry", "گۆشت و مریشک", "لحوم ودواجن"], icon: "tabler:meat",
    children: [
      { key: "chicken", names: ["Chicken", "مریشک", "دجاج"], icon: "tabler:meat" },
      { key: "red_meat", names: ["Red meat", "گۆشتی سوور", "لحم أحمر"], icon: "tabler:meat" },
      { key: "sausages", names: ["Sausages", "سۆسیج", "نقانق"], icon: "tabler:sausage" },
    ],
  },
  {
    key: "pantry", names: ["Canned & dry food", "خۆراکی وشک و قوتوو", "أغذية جافة ومعلبات"], icon: "tabler:box",
    children: [
      { key: "rice", names: ["Rice & grains", "برنج و دانەوێڵە", "أرز وحبوب"], icon: "tabler:wheat" },
      { key: "pasta", names: ["Pasta", "مەعکەرۆنە", "معكرونة"], icon: "tabler:bowl" },
      { key: "canned", names: ["Canned food", "قوتوو", "معلبات"], icon: "tabler:box" },
      { key: "oil", names: ["Oil", "ڕۆن", "زيت"], icon: "tabler:droplet" },
    ],
  },
  {
    key: "tea", names: ["Tea, coffee & sugar", "چا، قاوە و شەکر", "شاي وقهوة وسكر"], icon: "tabler:coffee",
    children: [
      { key: "tea_only", names: ["Tea", "چا", "شاي"], icon: "tabler:cup" },
      { key: "coffee", names: ["Coffee", "قاوە", "قهوة"], icon: "tabler:coffee" },
      { key: "sugar", names: ["Sugar", "شەکر", "سكر"], icon: "tabler:cube" },
    ],
  },
  { key: "spices", names: ["Spices", "بەهارات", "بهارات"], icon: "tabler:pepper" },
  { key: "frozen", names: ["Frozen", "بەستووەکان", "مجمدات"], icon: "tabler:snowflake" },
  {
    key: "cleaning", names: ["Cleaning", "پاککەرەوەکان", "منظفات"], icon: "tabler:spray",
    children: [
      { key: "detergents", names: ["Detergents", "پاککەرەوە", "مساحيق ومنظفات"], icon: "tabler:spray" },
      { key: "paper", names: ["Paper & tissues", "کاغەز و کلینکس", "ورق ومناديل"], icon: "tabler:paper-bag" },
    ],
  },
  {
    key: "care", names: ["Personal care", "پاکوخاوێنی کەسی", "عناية شخصية"], icon: "tabler:bath",
    children: [
      { key: "soap", names: ["Shampoo & soap", "شامپۆ و سابوون", "شامبو وصابون"], icon: "tabler:bath" },
      { key: "dental", names: ["Dental care", "ددان", "العناية بالأسنان"], icon: "tabler:dental" },
    ],
  },
  {
    key: "baby", names: ["Baby", "منداڵ", "أطفال"], icon: "tabler:baby-carriage",
    children: [
      { key: "diapers", names: ["Diapers", "پەمپەرز", "حفاضات"], icon: "tabler:baby-carriage" },
      { key: "baby_food", names: ["Baby food", "خۆراکی منداڵ", "أغذية أطفال"], icon: "tabler:baby-bottle" },
    ],
  },
  { key: "household", names: ["Household", "پێداویستی ماڵ", "أدوات منزلية"], icon: "tabler:home" },
  { key: "cigarettes", names: ["Cigarettes", "جگەرە", "سجائر"], icon: "tabler:smoking" },
  { key: "mobile", names: ["Mobile cards", "کارتی مۆبایل", "كروت الهاتف"], icon: "tabler:device-mobile" },
];

// A mini market stocks the everyday basics, without the finer groupings
const MINI_MARKET = ["beverages", "snacks", "dairy", "bakery", "produce", "pantry", "tea", "frozen", "cleaning", "care", "cigarettes", "mobile"];

export const starterTemplates = (type: ShopType): StarterTemplate[] => {
  if (type === "other") return [];
  const list = type === "supermarket" ? ALL : ALL.filter((t) => MINI_MARKET.includes(t.key)).map(({ children: _, ...t }) => t);
  return list.map((t, i) => ({ ...t, color: COLORS[i % COLORS.length] }));
};

export const pickName = (names: Names, lang: string) => (lang === "ku" ? names[1] : lang === "ar" ? names[2] : names[0]);
