// Roblox's avatar APIs, called through the signed-in Roblox page (see avatar.rs).

import { api } from './api';

const AVATAR = 'https://avatar.roblox.com';
const THUMBS = 'https://thumbnails.roblox.com';

export interface AvatarAsset {
  id: number;
  name: string;
  assetType: { id: number; name: string };
  currentVersionId?: number;
  meta?: Record<string, unknown>;
}
export interface Scales {
  height: number;
  width: number;
  head: number;
  depth: number;
  proportion: number;
  bodyType: number;
}
export type BodyPart =
  'headColor3' | 'torsoColor3' | 'rightArmColor3' | 'leftArmColor3' | 'rightLegColor3' | 'leftLegColor3';
export type BodyColors = Record<BodyPart, string>;
export interface Avatar {
  scales: Scales;
  playerAvatarType: 'R6' | 'R15';
  bodyColor3s: BodyColors;
  assets: AvatarAsset[];
}
export interface Rules {
  scales: Record<
    'height' | 'width' | 'head' | 'proportion' | 'bodyType',
    { min: number; max: number; increment: number }
  >;
  wearableAssetTypes: { id: number; name: string; maxNumber: number }[];
  bodyColorsPalette: { brickColorId: number; hexColor: string; name: string }[];
}
export interface Me {
  id: number;
  name: string;
  displayName: string;
}
export interface InventoryItem {
  assetId: number;
  assetName: string;
}
export interface Outfit {
  id: number;
  name: string;
  isEditable: boolean;
}
export interface OutfitDetails {
  id: number;
  name: string;
  assets: AvatarAsset[];
  bodyColors: Record<string, number>;
  scale: Scales;
  playerAvatarType: 'R6' | 'R15';
}
interface Thumb {
  targetId: number;
  state: string;
  imageUrl: string | null;
}

export const BODY_PARTS: { key: BodyPart; label: string }[] = [
  { key: 'headColor3', label: 'Head' },
  { key: 'torsoColor3', label: 'Torso' },
  { key: 'leftArmColor3', label: 'Left arm' },
  { key: 'rightArmColor3', label: 'Right arm' },
  { key: 'leftLegColor3', label: 'Left leg' },
  { key: 'rightLegColor3', label: 'Right leg' },
];

/** Inventory categories, as Roblox asset type ids. */
export const CATEGORIES: { id: string; label: string; types: number[] }[] = [
  { id: 'hats', label: 'Hats', types: [8] },
  { id: 'hair', label: 'Hair', types: [41] },
  { id: 'face', label: 'Face accessories', types: [42] },
  { id: 'neck', label: 'Neck', types: [43] },
  { id: 'shoulder', label: 'Shoulder', types: [44] },
  { id: 'front', label: 'Front', types: [45] },
  { id: 'back', label: 'Back', types: [46] },
  { id: 'waist', label: 'Waist', types: [47] },
  { id: 'layered', label: 'Layered clothing', types: [64, 65, 66, 67, 68, 69, 72] },
  { id: 'shoes', label: 'Shoes', types: [70, 71] },
  { id: 'classic', label: 'Shirts, pants & T-shirts', types: [11, 12, 2] },
  { id: 'heads', label: 'Heads & faces', types: [79, 17, 18] },
  { id: 'makeup', label: 'Makeup', types: [88, 89, 90] },
  { id: 'gear', label: 'Gear', types: [19] },
];

function query(base: string, params: Record<string, string | number | undefined>) {
  const u = new URL(base);
  for (const [k, v] of Object.entries(params)) if (v !== undefined) u.searchParams.set(k, String(v));
  return u.toString();
}

async function thumbs(path: string, idKey: string, ids: number[], size: string): Promise<Map<number, string>> {
  const out = new Map<number, string>();
  for (let i = 0; i < ids.length; i += 100) {
    const chunk = ids.slice(i, i + 100);
    const r = await api.avatarApi<{ data: Thumb[] }>(
      'GET',
      query(`${THUMBS}${path}`, { [idKey]: chunk.join(','), size, format: 'Png', isCircular: 'false' }),
    );
    for (const t of r.data) if (t.imageUrl && t.state === 'Completed') out.set(t.targetId, t.imageUrl);
  }
  return out;
}

export const avatar = {
  me: () => api.avatarApi<Me>('GET', 'https://users.roblox.com/v1/users/authenticated'),
  get: () => api.avatarApi<Avatar>('GET', `${AVATAR}/v2/avatar/avatar`),
  rules: () => api.avatarApi<Rules>('GET', `${AVATAR}/v1/avatar-rules`),
  wear: (assets: AvatarAsset[]) =>
    api.avatarApi<{ success: boolean; invalidAssetIds?: number[] }>('POST', `${AVATAR}/v2/avatar/set-wearing-assets`, {
      assets: assets.map((a) => (a.meta ? { id: a.id, meta: a.meta } : { id: a.id })),
    }),
  setType: (type: 'R6' | 'R15') =>
    api.avatarApi('POST', `${AVATAR}/v1/avatar/set-player-avatar-type`, { playerAvatarType: type }),
  setScales: (s: Scales) => api.avatarApi('POST', `${AVATAR}/v1/avatar/set-scales`, s),
  setColors: (c: BodyColors) => api.avatarApi('POST', `${AVATAR}/v2/avatar/set-body-colors`, c),
  setColorIds: (c: Record<string, number>) => api.avatarApi('POST', `${AVATAR}/v1/avatar/set-body-colors`, c),
  redraw: () => api.avatarApi('POST', `${AVATAR}/v1/avatar/redraw-thumbnail`, {}),
  /** The full-body render; `null` while Roblox is still drawing it. */
  render: async (userId: number) => {
    const r = await api.avatarApi<{ data: Thumb[] }>(
      'GET',
      query(`${THUMBS}/v1/users/avatar`, { userIds: userId, size: '420x420', format: 'Png', isCircular: 'false' }),
    );
    const t = r.data[0];
    return t?.state === 'Completed' ? t.imageUrl : null;
  },
  assetThumbs: (ids: number[]) => thumbs('/v1/assets', 'assetIds', ids, '150x150'),
  outfitThumbs: (ids: number[]) => thumbs('/v1/users/outfits', 'userOutfitIds', ids, '150x150'),
  inventory: (userId: number, types: number[], cursor?: string) =>
    api.avatarApi<{ data: InventoryItem[]; nextPageCursor: string | null }>(
      'GET',
      query(`https://inventory.roblox.com/v2/users/${userId}/inventory`, {
        assetTypes: types.map((t) => TYPE_NAMES[t] ?? t).join(','),
        limit: 100,
        sortOrder: 'Desc',
        cursor,
      }),
    ),
  outfits: (userId: number, page = 1) =>
    api.avatarApi<{ data: Outfit[]; total?: number }>(
      'GET',
      query(`${AVATAR}/v2/avatar/users/${userId}/outfits`, { outfitType: 'Avatar', page, itemsPerPage: 50 }),
    ),
  outfitDetails: (id: number) => api.avatarApi<OutfitDetails>('GET', `${AVATAR}/v1/outfits/${id}/details`),
  /** An item's name and type, from the catalog. */
  details: (id: number) =>
    api.avatarApi<{ AssetId: number; Name: string; AssetTypeId: number }>(
      'GET',
      `https://economy.roblox.com/v2/assets/${id}/details`,
    ),
};

/** Roblox's enum names for asset types, as the inventory API wants them. */
const TYPE_NAMES: Record<number, string> = {
  2: 'TShirt',
  8: 'Hat',
  11: 'Shirt',
  12: 'Pants',
  17: 'Head',
  18: 'Face',
  19: 'Gear',
  41: 'HairAccessory',
  42: 'FaceAccessory',
  43: 'NeckAccessory',
  44: 'ShoulderAccessory',
  45: 'FrontAccessory',
  46: 'BackAccessory',
  47: 'WaistAccessory',
  64: 'TShirtAccessory',
  65: 'ShirtAccessory',
  66: 'PantsAccessory',
  67: 'JacketAccessory',
  68: 'SweaterAccessory',
  69: 'ShortsAccessory',
  70: 'LeftShoeAccessory',
  71: 'RightShoeAccessory',
  72: 'DressSkirtAccessory',
  79: 'DynamicHead',
  88: 'FaceMakeup',
  89: 'LipMakeup',
  90: 'EyeMakeup',
};

/** An asset id from a number or a catalog / marketplace link. */
export function parseAssetId(input: string): number | null {
  const s = input.trim();
  if (/^\d{1,20}$/.test(s)) return Number(s);
  const m = s.match(/roblox\.com\/(?:catalog|library|marketplace\/asset)\/(\d{1,20})/i);
  return m ? Number(m[1]) : null;
}
