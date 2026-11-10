local t = 0

function setup()
    screen:fill(16, 16, 28)
end

function loop()
    screen:fill(16, 16, 28)

    local cx = math.floor(screen.width / 2 + math.cos(t * 0.05) * 80)
    local cy = math.floor(screen.height / 2 + math.sin(t * 0.07) * 50)

    for dy = -12, 12 do
        for dx = -12, 12 do
            if dx * dx + dy * dy <= 144 then
                local r = (cx + dx) % 256
                local g = (cy + dy) % 256
                local b = 220
                screen:set_pixel(cx + dx, cy + dy, r, g, b)
            end
        end
    end

    t = t + 1
end
