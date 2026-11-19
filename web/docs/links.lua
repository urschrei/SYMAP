-- Pandoc filter for the published guides: link to the other published guides
-- as pages, to everything else in the repository on GitHub, and take the
-- page title from the first heading.
local repo = 'https://github.com/urschrei/SYMAP/blob/main/'
local published = { ['importing.md'] = true, ['your-own-data.md'] = true }

function Link(el)
  local target = el.target
  if target:match('^%a[%w+.-]*:') or target:match('^#') then return el end
  local file, anchor = target:match('^([^#]*)(#?.*)$')
  if published[file] then
    el.target = file:gsub('%.md$', '.html') .. anchor
  else
    el.target = repo .. 'docs/' .. file .. anchor
  end
  return el
end

function Pandoc(doc)
  for _, block in ipairs(doc.blocks) do
    if block.t == 'Header' and block.level == 1 then
      doc.meta.pagetitle = pandoc.utils.stringify(block)
      break
    end
  end
  return doc
end

-- Let wide tables scroll on their own rather than widen the page.
function Table(el)
  return pandoc.Div({ el }, pandoc.Attr('', { 'table-wrap' }))
end
