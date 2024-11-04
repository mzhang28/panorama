// Cribbed from https://github.com/cinnyapp/folds/blob/d5e4f8a3d48ae70d54e77a05808f5200e74072fb/src/theme/util.ts#L1
// Just going to use this for now until I figure out my own system
export const pxToRem = (px: number) => Number.parseFloat((px / 16).toFixed(4));
export const toRem = (px: number) => `${pxToRem(px)}rem`;
