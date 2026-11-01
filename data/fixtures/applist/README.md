# GetAppList fixtures

Pages from `IStoreService/GetAppList/v1` (`include_games=true`, paged with `last_appid`). **Hand-written** in the response shape from Steam's Web API documentation, because fetching real pages needs the API key. Replace them with a real capture after the first pipeline run.

The last page has no `have_more_results`, as Steam omits it when there's nothing more.
