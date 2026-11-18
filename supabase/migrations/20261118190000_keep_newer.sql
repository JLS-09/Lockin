-- The newer change wins (phase 5, step 3).
--
-- The sync sends every row a computer changed. When two computers changed the
-- same row (one of them offline for a while), the one with the later
-- `updated_at` must win, whichever reaches the cloud last. This trigger makes
-- an update with an older `updated_at` do nothing, so a computer coming back
-- online can't overwrite a newer change; it fetches that change instead.
--
-- Triggers of the same kind run in name order: "keep_newer" runs before
-- "set_synced_at", so a skipped update doesn't touch synced_at either.
-- `updated_at` is RFC 3339 text in UTC with seconds, so it compares as text.

create function public.lockin_keep_newer()
returns trigger
language plpgsql
set search_path = ''
as $$
begin
    if new.updated_at < old.updated_at then
        return null;   -- keep the row as it is
    end if;
    return new;
end;
$$;

do $$
declare
    t text;
begin
    foreach t in array array[
        'course', 'calendar_source', 'filter_rule', 'history_import', 'class_block',
        'activity_template', 'activity', 'deadline', 'block_profile', 'block_app',
        'block_site', 'session_series', 'study_session', 'session_event',
        'app_closed', 'points_ledger'
    ] loop
        execute format(
            'create trigger keep_newer before update on public.%I
             for each row execute function public.lockin_keep_newer()', t);
    end loop;
end;
$$;
