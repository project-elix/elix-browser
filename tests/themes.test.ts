import {describe,it,expect} from 'vitest';
import {contrast,defaultAppearance,foreground,presets,themeVariables} from '../src/themes/theme';
describe('theme readability',()=>{
 it('keeps text and interactive accents readable for every preset',()=>{for(const preset of presets){const t=themeVariables({...defaultAppearance,...preset});expect(contrast(t['--text'],t['--bg'])).toBeGreaterThanOrEqual(4.5);expect(contrast(t['--accent'],t['--bg'])).toBeGreaterThanOrEqual(4.5);expect(contrast(t['--accent'],t['--on-accent'])).toBeGreaterThanOrEqual(4.5)}});
 it('corrects accents identical to the background without changing the chosen saved value',()=>{for(const background of ['#000000','#ffffff','#777777','#ff0000','#0000ff']){const a={...defaultAppearance,accent:background,background};const t=themeVariables(a);expect(contrast(t['--accent'],background)).toBeGreaterThanOrEqual(4.5);expect(a.accent).toBe(background)}});
 it('chooses a light or dark foreground by contrast',()=>{expect(foreground('#ffffff')).toBe('#000000');expect(foreground('#080b12')).toBe('#ffffff')});
});
