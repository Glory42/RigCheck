# ProtonDB fixtures

`reports.json`: 20 real reports, verbatim, from ProtonDB's `reports_oct1_2026.tar.gz` dump ([bdefore/protondb-data](https://github.com/bdefore/protondb-data)). ProtonDB's data is licensed under the [ODbL](http://opendatacommons.org/licenses/odbl/).

Picked to exercise the tier rule (ROADMAP §9): every report for Lara Croft GO (540840), Act of War: High Treason (9760), Chompy Chomp Chomp (292570) and Bye Sweet Carole (2428980), plus one report on a native Linux build (3493540), and one Way of the Samurai 3 (427510) report whose `appId` is a number, as about 8% of the dump's are. Tests treat 2026-10-01 as "now".
