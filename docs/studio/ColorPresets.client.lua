--[[
	Colour presets for experiences you build in Roblox Studio.

	Vesper doesn't inject scripts into Roblox: running code inside games you
	don't own needs an exploit executor and gets accounts banned. In your own
	experiences, though, colour grading is a normal, supported feature. This
	LocalScript applies the same presets with Roblox's ColorCorrectionEffect.

	How to use:
	  1. In Studio, put this LocalScript in StarterPlayer > StarterPlayerScripts.
	  2. Pick a preset below (or call ColorPresets.apply("Night") from your own code).
	  3. Press Play.
]]

local Lighting = game:GetService("Lighting")

local Config = {
	Preset = "Saturated",
	Ambience = false,

	Presets = {
		Pink      = { Saturation = 0.8,  Contrast = 0.1,  Brightness = 0.03,  Tint = Color3.fromRGB(255, 180, 220) },
		Purple    = { Saturation = 0.75, Contrast = 0.12, Brightness = 0.02,  Tint = Color3.fromRGB(220, 180, 255) },
		Blue      = { Saturation = 0.7,  Contrast = 0.1,  Brightness = 0.01,  Tint = Color3.fromRGB(180, 210, 255) },
		Red       = { Saturation = 0.7,  Contrast = 0.15, Brightness = 0.02,  Tint = Color3.fromRGB(255, 185, 185) },
		Green     = { Saturation = 0.65, Contrast = 0.1,  Brightness = 0.02,  Tint = Color3.fromRGB(200, 255, 210) },
		Warm      = { Saturation = 0.85, Contrast = 0.08, Brightness = 0.04,  Tint = Color3.fromRGB(255, 230, 195) },
		Cold      = { Saturation = 0.6,  Contrast = 0.12, Brightness = 0,     Tint = Color3.fromRGB(200, 220, 255) },
		Saturated = { Saturation = 1,    Contrast = 0,    Brightness = 0,     Tint = Color3.fromRGB(255, 255, 255) },
		Night     = { Saturation = -0.3, Contrast = 0.2,  Brightness = -0.05, Tint = Color3.fromRGB(180, 200, 255) },
	},

	AmbienceSettings = {
		Ambient = Color3.fromRGB(120, 120, 130),
		OutdoorAmbient = Color3.fromRGB(150, 150, 160),
		Brightness = 2,
		ClockTime = 14,
		FogColor = Color3.fromRGB(180, 180, 190),
		FogEnd = 5000,
		FogStart = 1000,
	},
}

local EFFECT_NAME = "VesperColorPreset"

local ColorPresets = {}

function ColorPresets.apply(name: string)
	local preset = Config.Presets[name]
	if not preset then
		warn(("Unknown colour preset %q"):format(tostring(name)))
		return
	end
	local effect = Lighting:FindFirstChild(EFFECT_NAME)
	if not effect then
		effect = Instance.new("ColorCorrectionEffect")
		effect.Name = EFFECT_NAME
		effect.Parent = Lighting
	end
	effect.Saturation = preset.Saturation
	effect.Contrast = preset.Contrast
	effect.Brightness = preset.Brightness
	effect.TintColor = preset.Tint
end

function ColorPresets.applyAmbience()
	local a = Config.AmbienceSettings
	Lighting.Ambient = a.Ambient
	Lighting.OutdoorAmbient = a.OutdoorAmbient
	Lighting.Brightness = a.Brightness
	Lighting.ClockTime = a.ClockTime
	Lighting.FogColor = a.FogColor
	Lighting.FogEnd = a.FogEnd
	Lighting.FogStart = a.FogStart
end

ColorPresets.apply(Config.Preset)
if Config.Ambience then
	ColorPresets.applyAmbience()
end

