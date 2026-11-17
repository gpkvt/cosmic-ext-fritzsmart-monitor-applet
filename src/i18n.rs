//! Multilingual interface translations with desktop locale detection.

use std::env;

/// Keep an explicitly selected supported language instead of silently resetting it to system.
pub fn language_setting(value: &str) -> &'static str {
    match value {
        "en" | "de" | "es" | "nl" | "fr" | "it" | "sv" | "pt" | "pl" | "da" | "no" | "ja"
        | "ko" | "system" => match value {
            "en" => "en",
            "de" => "de",
            "es" => "es",
            "nl" => "nl",
            "fr" => "fr",
            "it" => "it",
            "sv" => "sv",
            "pt" => "pt",
            "pl" => "pl",
            "da" => "da",
            "no" => "no",
            "ja" => "ja",
            "ko" => "ko",
            _ => "system",
        },
        _ => "system",
    }
}

/// Resolve the configured language. `system` follows the desktop locale.
pub fn language(setting: &str) -> &'static str {
    fn from_locale(value: &str) -> Option<&'static str> {
        let value = value.to_ascii_lowercase();
        let value = value.split(':').next().unwrap_or(&value);
        let value = value.split(['_', '-', '.']).next().unwrap_or(value);
        match value {
            "de" => Some("de"),
            "en" => Some("en"),
            "es" => Some("es"),
            "nl" => Some("nl"),
            "fr" => Some("fr"),
            "it" => Some("it"),
            "sv" => Some("sv"),
            "pt" => Some("pt"),
            "pl" => Some("pl"),
            "da" => Some("da"),
            "no" | "nb" | "nn" => Some("no"),
            "ja" => Some("ja"),
            "ko" => Some("ko"),
            _ => None,
        }
    }

    if setting != "system"
        && let Some(language) = from_locale(setting)
    {
        return language;
    }
    for key in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = env::var(key)
            && let Some(language) = from_locale(&value)
        {
            return language;
        }
    }
    "en"
}

/// Translate an English source string; unknown strings fall back to English.
pub fn tr(setting: &str, key: &'static str) -> &'static str {
    let lang = language(setting);
    if lang != "de" {
        return match (lang, key) {
            // Spanish
            (
                "es",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish and Norwegian.",
            ) => {
                "El sistema sigue el idioma del escritorio. Idiomas disponibles: inglés, alemán, español, neerlandés, francés, italiano, sueco, portugués, polaco, danés y noruego."
            }
            (
                "nl",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish and Norwegian.",
            ) => {
                "Het systeem volgt de taal van je bureaublad. Beschikbare talen: Engels, Duits, Spaans, Nederlands, Frans, Italiaans, Zweeds, Portugees, Pools, Deens en Noors."
            }
            (
                "fr",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish and Norwegian.",
            ) => {
                "Le système suit la langue de votre bureau. Langues disponibles : anglais, allemand, espagnol, néerlandais, français, italien, suédois, portugais, polonais, danois et norvégien."
            }
            (
                "it",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish and Norwegian.",
            ) => {
                "Il sistema segue la lingua del desktop. Lingue disponibili: inglese, tedesco, spagnolo, olandese, francese, italiano, svedese, portoghese, polacco, danese e norvegese."
            }
            (
                "sv",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish and Norwegian.",
            ) => {
                "Systemet följer skrivbordets språk. Tillgängliga språk: engelska, tyska, spanska, nederländska, franska, italienska, svenska, portugisiska, polska, danska och norska."
            }
            (
                "pt",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish and Norwegian.",
            ) => {
                "O sistema segue o idioma do ambiente de trabalho. Idiomas disponíveis: inglês, alemão, espanhol, neerlandês, francês, italiano, sueco, português, polaco, dinamarquês e norueguês."
            }
            (
                "pl",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish and Norwegian.",
            ) => {
                "System używa języka pulpitu. Dostępne języki: angielski, niemiecki, hiszpański, niderlandzki, francuski, włoski, szwedzki, portugalski, polski, duński i norweski."
            }
            (
                "da",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish and Norwegian.",
            ) => {
                "Systemet følger sproget på skrivebordet. Tilgængelige sprog: engelsk, tysk, spansk, nederlandsk, fransk, italiensk, svensk, portugisisk, polsk, dansk og norsk."
            }
            (
                "no",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish and Norwegian.",
            ) => {
                "Systemet følger språket på skrivebordet. Tilgjengelige språk: engelsk, tysk, spansk, nederlandsk, fransk, italiensk, svensk, portugisisk, polsk, dansk og norsk."
            }
            ("es", "Settings") => "Ajustes",
            ("es", "Configure your power monitor") => "Configura tu monitor de energía",
            ("es", "GENERAL") => "GENERAL",
            ("es", "Language") => "Idioma",
            ("es", "Interface language") => "Idioma de la interfaz",
            ("es", "System default") => "Idioma del sistema",
            ("es", "English") => "Inglés",
            ("es", "Deutsch") => "Alemán",
            ("es", "Panel") => "Panel",
            ("es", "Graph history") => "Historial de gráficas",
            ("es", "Automatic refresh") => "Actualización automática",
            ("es", "Weather") => "Tiempo",
            ("es", "Connection") => "Conexión",
            ("es", "Devices") => "Dispositivos",
            ("es", "Menu order") => "Orden del menú",
            ("es", "Panel order") => "Orden del panel",
            ("es", "Diagnostics") => "Diagnóstico",
            ("es", "Panel text size") => "Tamaño del texto del panel",
            ("es", "Panel item spacing") => "Espaciado de los elementos del panel",
            ("es", "Refresh") => "Actualizar",
            ("es", "Refresh now") => "Actualizar ahora",
            ("es", "Load devices") => "Cargar dispositivos",
            ("es", "Search devices") => "Buscar dispositivos",
            ("es", "No devices match your search") => "Ningún dispositivo coincide con la búsqueda",
            ("es", "Temperature") => "Temperatura",
            ("es", "Average temperature") => "Temperatura media",
            ("es", "Wind speed") => "Velocidad del viento",
            ("es", "Direction") => "Dirección",
            ("es", "Pressure") => "Presión",
            ("es", "Humidity") => "Humedad",
            ("es", "Clouds") => "Nubosidad",
            ("es", "Visibility") => "Visibilidad",
            ("es", "Sunrise") => "Amanecer",
            ("es", "Sunset") => "Atardecer",
            ("es", "Feels like") => "Sensación térmica",
            ("es", "Now") => "Ahora",
            ("es", "DOWNLOAD") => "DESCARGA",
            ("es", "UPLOAD") => "SUBIDA",
            ("es", "POWER (W)") => "POTENCIA (W)",
            ("es", "SETTINGS") => "AJUSTES",
            ("es", "QUICK SETTINGS") => "AJUSTES RÁPIDOS",
            ("es", "Connection error") => "Error de conexión",
            ("es", "Connection failed") => "Conexión fallida",
            ("es", "Settings saved") => "Ajustes guardados",
            ("es", "Enabled") => "Activado",
            ("es", "Disabled") => "Desactivado",
            ("es", "On") => "Encendido",
            ("es", "Off") => "Apagado",
            ("es", "Upper limit") => "Límite superior",
            ("es", "Lower limit") => "Límite inferior",
            ("es", "Temperature alerts") => "Alertas de temperatura",
            ("es", "Graph") => "Gráfica",
            ("es", "Line") => "Línea",
            ("es", "Bar") => "Barras",
            ("es", "Forecast unavailable") => "Pronóstico no disponible",
            ("es", "Loading forecast…") => "Cargando pronóstico…",

            // Dutch (Netherlands)
            ("nl", "Settings") => "Instellingen",
            ("nl", "Configure your power monitor") => "Stel je energiemonitor in",
            ("nl", "GENERAL") => "ALGEMEEN",
            ("nl", "Language") => "Taal",
            ("nl", "Interface language") => "Interfacetaal",
            ("nl", "System default") => "Systeemtaal",
            ("nl", "English") => "Engels",
            ("nl", "Deutsch") => "Duits",
            ("nl", "Panel") => "Paneel",
            ("nl", "Graph history") => "Grafiekgeschiedenis",
            ("nl", "Automatic refresh") => "Automatisch vernieuwen",
            ("nl", "Weather") => "Weer",
            ("nl", "Connection") => "Verbinding",
            ("nl", "Devices") => "Apparaten",
            ("nl", "Menu order") => "Menuvolgorde",
            ("nl", "Panel order") => "Paneelvolgorde",
            ("nl", "Diagnostics") => "Diagnostiek",
            ("nl", "Panel text size") => "Tekstgrootte van paneel",
            ("nl", "Panel item spacing") => "Afstand tussen paneelitems",
            ("nl", "Refresh") => "Vernieuwen",
            ("nl", "Refresh now") => "Nu vernieuwen",
            ("nl", "Load devices") => "Apparaten laden",
            ("nl", "Search devices") => "Apparaten zoeken",
            ("nl", "No devices match your search") => "Geen apparaten gevonden",
            ("nl", "Temperature") => "Temperatuur",
            ("nl", "Average temperature") => "Gemiddelde temperatuur",
            ("nl", "Wind speed") => "Windsnelheid",
            ("nl", "Direction") => "Richting",
            ("nl", "Pressure") => "Luchtdruk",
            ("nl", "Humidity") => "Luchtvochtigheid",
            ("nl", "Clouds") => "Bewolking",
            ("nl", "Visibility") => "Zichtbaarheid",
            ("nl", "Sunrise") => "Zonsopkomst",
            ("nl", "Sunset") => "Zonsondergang",
            ("nl", "Feels like") => "Gevoelstemperatuur",
            ("nl", "Now") => "Nu",
            ("nl", "DOWNLOAD") => "DOWNLOAD",
            ("nl", "UPLOAD") => "UPLOAD",
            ("nl", "POWER (W)") => "VERMOGEN (W)",
            ("nl", "SETTINGS") => "INSTELLINGEN",
            ("nl", "QUICK SETTINGS") => "SNELLE INSTELLINGEN",
            ("nl", "Connection error") => "Verbindingsfout",
            ("nl", "Connection failed") => "Verbinding mislukt",
            ("nl", "Settings saved") => "Instellingen opgeslagen",
            ("nl", "Enabled") => "Ingeschakeld",
            ("nl", "Disabled") => "Uitgeschakeld",
            ("nl", "On") => "Aan",
            ("nl", "Off") => "Uit",
            ("nl", "Upper limit") => "Bovengrens",
            ("nl", "Lower limit") => "Ondergrens",
            ("nl", "Temperature alerts") => "Temperatuurwaarschuwingen",
            ("nl", "Graph") => "Grafiek",
            ("nl", "Line") => "Lijn",
            ("nl", "Bar") => "Staaf",
            ("nl", "Forecast unavailable") => "Verwachting niet beschikbaar",
            ("nl", "Loading forecast…") => "Verwachting laden…",

            // French
            ("fr", "Settings") => "Paramètres",
            ("fr", "Configure your power monitor") => "Configurer votre moniteur d’énergie",
            ("fr", "GENERAL") => "GÉNÉRAL",
            ("fr", "Language") => "Langue",
            ("fr", "Interface language") => "Langue de l’interface",
            ("fr", "System default") => "Langue du système",
            ("fr", "English") => "Anglais",
            ("fr", "Deutsch") => "Allemand",
            ("fr", "Panel") => "Panneau",
            ("fr", "Graph history") => "Historique des graphiques",
            ("fr", "Automatic refresh") => "Actualisation automatique",
            ("fr", "Weather") => "Météo",
            ("fr", "Connection") => "Connexion",
            ("fr", "Devices") => "Appareils",
            ("fr", "Menu order") => "Ordre du menu",
            ("fr", "Panel order") => "Ordre du panneau",
            ("fr", "Diagnostics") => "Diagnostic",
            ("fr", "Panel text size") => "Taille du texte du panneau",
            ("fr", "Panel item spacing") => "Espacement des éléments du panneau",
            ("fr", "Refresh") => "Actualiser",
            ("fr", "Refresh now") => "Actualiser maintenant",
            ("fr", "Load devices") => "Charger les appareils",
            ("fr", "Search devices") => "Rechercher des appareils",
            ("fr", "No devices match your search") => "Aucun appareil ne correspond à la recherche",
            ("fr", "Temperature") => "Température",
            ("fr", "Average temperature") => "Température moyenne",
            ("fr", "Wind speed") => "Vitesse du vent",
            ("fr", "Direction") => "Direction",
            ("fr", "Pressure") => "Pression",
            ("fr", "Humidity") => "Humidité",
            ("fr", "Clouds") => "Nuages",
            ("fr", "Visibility") => "Visibilité",
            ("fr", "Sunrise") => "Lever du soleil",
            ("fr", "Sunset") => "Coucher du soleil",
            ("fr", "Feels like") => "Ressenti",
            ("fr", "Now") => "Maintenant",
            ("fr", "DOWNLOAD") => "TÉLÉCHARGEMENT",
            ("fr", "UPLOAD") => "ENVOI",
            ("fr", "POWER (W)") => "PUISSANCE (W)",
            ("fr", "SETTINGS") => "PARAMÈTRES",
            ("fr", "QUICK SETTINGS") => "RÉGLAGES RAPIDES",
            ("fr", "Connection error") => "Erreur de connexion",
            ("fr", "Connection failed") => "Échec de la connexion",
            ("fr", "Settings saved") => "Paramètres enregistrés",
            ("fr", "Enabled") => "Activé",
            ("fr", "Disabled") => "Désactivé",
            ("fr", "On") => "Activé",
            ("fr", "Off") => "Désactivé",
            ("fr", "Upper limit") => "Limite supérieure",
            ("fr", "Lower limit") => "Limite inférieure",
            ("fr", "Temperature alerts") => "Alertes de température",
            ("fr", "Graph") => "Graphique",
            ("fr", "Line") => "Courbe",
            ("fr", "Bar") => "Barres",
            ("fr", "Forecast unavailable") => "Prévisions indisponibles",
            ("fr", "Loading forecast…") => "Chargement des prévisions…",

            // Italian
            ("it", "Settings") => "Impostazioni",
            ("it", "Configure your power monitor") => "Configura il monitor energetico",
            ("it", "GENERAL") => "GENERALE",
            ("it", "Language") => "Lingua",
            ("it", "Interface language") => "Lingua dell’interfaccia",
            ("it", "System default") => "Lingua di sistema",
            ("it", "English") => "Inglese",
            ("it", "Deutsch") => "Tedesco",
            ("it", "Panel") => "Pannello",
            ("it", "Graph history") => "Cronologia dei grafici",
            ("it", "Automatic refresh") => "Aggiornamento automatico",
            ("it", "Weather") => "Meteo",
            ("it", "Connection") => "Connessione",
            ("it", "Devices") => "Dispositivi",
            ("it", "Menu order") => "Ordine del menu",
            ("it", "Panel order") => "Ordine del pannello",
            ("it", "Diagnostics") => "Diagnostica",
            ("it", "Panel text size") => "Dimensione del testo del pannello",
            ("it", "Panel item spacing") => "Spaziatura degli elementi del pannello",
            ("it", "Refresh") => "Aggiorna",
            ("it", "Refresh now") => "Aggiorna ora",
            ("it", "Load devices") => "Carica dispositivi",
            ("it", "Search devices") => "Cerca dispositivi",
            ("it", "No devices match your search") => "Nessun dispositivo corrisponde alla ricerca",
            ("it", "Temperature") => "Temperatura",
            ("it", "Average temperature") => "Temperatura media",
            ("it", "Wind speed") => "Velocità del vento",
            ("it", "Direction") => "Direzione",
            ("it", "Pressure") => "Pressione",
            ("it", "Humidity") => "Umidità",
            ("it", "Clouds") => "Nuvolosità",
            ("it", "Visibility") => "Visibilità",
            ("it", "Sunrise") => "Alba",
            ("it", "Sunset") => "Tramonto",
            ("it", "Feels like") => "Percepita",
            ("it", "Now") => "Ora",
            ("it", "DOWNLOAD") => "DOWNLOAD",
            ("it", "UPLOAD") => "UPLOAD",
            ("it", "POWER (W)") => "POTENZA (W)",
            ("it", "SETTINGS") => "IMPOSTAZIONI",
            ("it", "QUICK SETTINGS") => "IMPOSTAZIONI RAPIDE",
            ("it", "Connection error") => "Errore di connessione",
            ("it", "Connection failed") => "Connessione non riuscita",
            ("it", "Settings saved") => "Impostazioni salvate",
            ("it", "Enabled") => "Attivato",
            ("it", "Disabled") => "Disattivato",
            ("it", "On") => "Acceso",
            ("it", "Off") => "Spento",
            ("it", "Upper limit") => "Limite superiore",
            ("it", "Lower limit") => "Limite inferiore",
            ("it", "Temperature alerts") => "Avvisi di temperatura",
            ("it", "Graph") => "Grafico",
            ("it", "Line") => "Linea",
            ("it", "Bar") => "Barre",
            ("it", "Forecast unavailable") => "Previsioni non disponibili",
            ("it", "Loading forecast…") => "Caricamento previsioni…",

            // Swedish
            ("sv", "Settings") => "Inställningar",
            ("sv", "Configure your power monitor") => "Konfigurera din effektmätare",
            ("sv", "GENERAL") => "ALLMÄNT",
            ("sv", "Language") => "Språk",
            ("sv", "Interface language") => "Gränssnittsspråk",
            ("sv", "System default") => "Systemspråk",
            ("sv", "English") => "Engelska",
            ("sv", "Deutsch") => "Tyska",
            ("sv", "Panel") => "Panel",
            ("sv", "Graph history") => "Diagramhistorik",
            ("sv", "Automatic refresh") => "Automatisk uppdatering",
            ("sv", "Weather") => "Väder",
            ("sv", "Connection") => "Anslutning",
            ("sv", "Devices") => "Enheter",
            ("sv", "Menu order") => "Menyordning",
            ("sv", "Panel order") => "Panelordning",
            ("sv", "Diagnostics") => "Diagnostik",
            ("sv", "Panel text size") => "Panelens textstorlek",
            ("sv", "Panel item spacing") => "Avstånd mellan panelobjekt",
            ("sv", "Refresh") => "Uppdatera",
            ("sv", "Refresh now") => "Uppdatera nu",
            ("sv", "Load devices") => "Läs in enheter",
            ("sv", "Search devices") => "Sök efter enheter",
            ("sv", "No devices match your search") => "Inga enheter matchar sökningen",
            ("sv", "Temperature") => "Temperatur",
            ("sv", "Average temperature") => "Medeltemperatur",
            ("sv", "Wind speed") => "Vindhastighet",
            ("sv", "Direction") => "Riktning",
            ("sv", "Pressure") => "Lufttryck",
            ("sv", "Humidity") => "Luftfuktighet",
            ("sv", "Clouds") => "Molnighet",
            ("sv", "Visibility") => "Sikt",
            ("sv", "Sunrise") => "Soluppgång",
            ("sv", "Sunset") => "Solnedgång",
            ("sv", "Feels like") => "Känns som",
            ("sv", "Now") => "Nu",
            ("sv", "DOWNLOAD") => "NEDLADDNING",
            ("sv", "UPLOAD") => "UPPLADDNING",
            ("sv", "POWER (W)") => "EFFEKT (W)",
            ("sv", "SETTINGS") => "INSTÄLLNINGAR",
            ("sv", "QUICK SETTINGS") => "SNABBINSTÄLLNINGAR",
            ("sv", "Connection error") => "Anslutningsfel",
            ("sv", "Connection failed") => "Anslutningen misslyckades",
            ("sv", "Settings saved") => "Inställningarna har sparats",
            ("sv", "Enabled") => "Aktiverad",
            ("sv", "Disabled") => "Inaktiverad",
            ("sv", "On") => "På",
            ("sv", "Off") => "Av",
            ("sv", "Upper limit") => "Övre gräns",
            ("sv", "Lower limit") => "Nedre gräns",
            ("sv", "Temperature alerts") => "Temperaturvarningar",
            ("sv", "Graph") => "Diagram",
            ("sv", "Line") => "Linje",
            ("sv", "Bar") => "Staplar",
            ("sv", "Forecast unavailable") => "Prognos saknas",
            ("sv", "Loading forecast…") => "Läser in prognos…",

            // Portuguese
            ("pt", "Settings") => "Definições",
            ("pt", "Configure your power monitor") => "Configure o monitor de energia",
            ("pt", "GENERAL") => "GERAL",
            ("pt", "Language") => "Idioma",
            ("pt", "Interface language") => "Idioma da interface",
            ("pt", "System default") => "Idioma do sistema",
            ("pt", "English") => "Inglês",
            ("pt", "Deutsch") => "Alemão",
            ("pt", "Panel") => "Painel",
            ("pt", "Graph history") => "Histórico dos gráficos",
            ("pt", "Automatic refresh") => "Atualização automática",
            ("pt", "Weather") => "Meteorologia",
            ("pt", "Connection") => "Ligação",
            ("pt", "Devices") => "Dispositivos",
            ("pt", "Menu order") => "Ordem do menu",
            ("pt", "Panel order") => "Ordem do painel",
            ("pt", "Diagnostics") => "Diagnóstico",
            ("pt", "Panel text size") => "Tamanho do texto do painel",
            ("pt", "Panel item spacing") => "Espaçamento dos itens do painel",
            ("pt", "Refresh") => "Atualizar",
            ("pt", "Refresh now") => "Atualizar agora",
            ("pt", "Load devices") => "Carregar dispositivos",
            ("pt", "Search devices") => "Procurar dispositivos",
            ("pt", "No devices match your search") => "Nenhum dispositivo corresponde à pesquisa",
            ("pt", "Temperature") => "Temperatura",
            ("pt", "Average temperature") => "Temperatura média",
            ("pt", "Wind speed") => "Velocidade do vento",
            ("pt", "Direction") => "Direção",
            ("pt", "Pressure") => "Pressão",
            ("pt", "Humidity") => "Humidade",
            ("pt", "Clouds") => "Nebulosidade",
            ("pt", "Visibility") => "Visibilidade",
            ("pt", "Sunrise") => "Nascer do sol",
            ("pt", "Sunset") => "Pôr do sol",
            ("pt", "Feels like") => "Sensação térmica",
            ("pt", "Now") => "Agora",
            ("pt", "DOWNLOAD") => "TRANSFERÊNCIA",
            ("pt", "UPLOAD") => "ENVIO",
            ("pt", "POWER (W)") => "POTÊNCIA (W)",
            ("pt", "SETTINGS") => "DEFINIÇÕES",
            ("pt", "QUICK SETTINGS") => "DEFINIÇÕES RÁPIDAS",
            ("pt", "Connection error") => "Erro de ligação",
            ("pt", "Connection failed") => "Falha na ligação",
            ("pt", "Settings saved") => "Definições guardadas",
            ("pt", "Enabled") => "Ativado",
            ("pt", "Disabled") => "Desativado",
            ("pt", "On") => "Ligado",
            ("pt", "Off") => "Desligado",
            ("pt", "Upper limit") => "Limite superior",
            ("pt", "Lower limit") => "Limite inferior",
            ("pt", "Temperature alerts") => "Alertas de temperatura",
            ("pt", "Graph") => "Gráfico",
            ("pt", "Line") => "Linha",
            ("pt", "Bar") => "Barras",
            ("pt", "Forecast unavailable") => "Previsão indisponível",
            ("pt", "Loading forecast…") => "A carregar previsão…",

            // Polish
            ("pl", "Settings") => "Ustawienia",
            ("pl", "Configure your power monitor") => "Skonfiguruj monitor energii",
            ("pl", "GENERAL") => "OGÓLNE",
            ("pl", "Language") => "Język",
            ("pl", "Interface language") => "Język interfejsu",
            ("pl", "System default") => "Język systemu",
            ("pl", "English") => "Angielski",
            ("pl", "Deutsch") => "Niemiecki",
            ("pl", "Panel") => "Panel",
            ("pl", "Graph history") => "Historia wykresów",
            ("pl", "Automatic refresh") => "Automatyczne odświeżanie",
            ("pl", "Weather") => "Pogoda",
            ("pl", "Connection") => "Połączenie",
            ("pl", "Devices") => "Urządzenia",
            ("pl", "Menu order") => "Kolejność menu",
            ("pl", "Panel order") => "Kolejność panelu",
            ("pl", "Diagnostics") => "Diagnostyka",
            ("pl", "Panel text size") => "Rozmiar tekstu panelu",
            ("pl", "Panel item spacing") => "Odstęp między elementami panelu",
            ("pl", "Refresh") => "Odśwież",
            ("pl", "Refresh now") => "Odśwież teraz",
            ("pl", "Load devices") => "Wczytaj urządzenia",
            ("pl", "Search devices") => "Szukaj urządzeń",
            ("pl", "No devices match your search") => "Brak urządzeń pasujących do wyszukiwania",
            ("pl", "Temperature") => "Temperatura",
            ("pl", "Average temperature") => "Średnia temperatura",
            ("pl", "Wind speed") => "Prędkość wiatru",
            ("pl", "Direction") => "Kierunek",
            ("pl", "Pressure") => "Ciśnienie",
            ("pl", "Humidity") => "Wilgotność",
            ("pl", "Clouds") => "Zachmurzenie",
            ("pl", "Visibility") => "Widoczność",
            ("pl", "Sunrise") => "Wschód słońca",
            ("pl", "Sunset") => "Zachód słońca",
            ("pl", "Feels like") => "Temperatura odczuwalna",
            ("pl", "Now") => "Teraz",
            ("pl", "DOWNLOAD") => "POBIERANIE",
            ("pl", "UPLOAD") => "WYSYŁANIE",
            ("pl", "POWER (W)") => "MOC (W)",
            ("pl", "SETTINGS") => "USTAWIENIA",
            ("pl", "QUICK SETTINGS") => "SZYBKIE USTAWIENIA",
            ("pl", "Connection error") => "Błąd połączenia",
            ("pl", "Connection failed") => "Połączenie nie powiodło się",
            ("pl", "Settings saved") => "Ustawienia zapisane",
            ("pl", "Enabled") => "Włączone",
            ("pl", "Disabled") => "Wyłączone",
            ("pl", "On") => "Włączone",
            ("pl", "Off") => "Wyłączone",
            ("pl", "Upper limit") => "Górny limit",
            ("pl", "Lower limit") => "Dolny limit",
            ("pl", "Temperature alerts") => "Alerty temperatury",
            ("pl", "Graph") => "Wykres",
            ("pl", "Line") => "Linia",
            ("pl", "Bar") => "Słupki",
            ("pl", "Forecast unavailable") => "Prognoza niedostępna",
            ("pl", "Loading forecast…") => "Wczytywanie prognozy…",

            // Danish
            ("da", "Settings") => "Indstillinger",
            ("da", "Configure your power monitor") => "Konfigurer din strømmåler",
            ("da", "GENERAL") => "GENERELT",
            ("da", "Language") => "Sprog",
            ("da", "Interface language") => "Grænsefladesprog",
            ("da", "System default") => "Systemsprog",
            ("da", "English") => "Engelsk",
            ("da", "Deutsch") => "Tysk",
            ("da", "Panel") => "Panel",
            ("da", "Graph history") => "Diagramhistorik",
            ("da", "Automatic refresh") => "Automatisk opdatering",
            ("da", "Weather") => "Vejr",
            ("da", "Connection") => "Forbindelse",
            ("da", "Devices") => "Enheder",
            ("da", "Menu order") => "Menurækkefølge",
            ("da", "Panel order") => "Panelrækkefølge",
            ("da", "Diagnostics") => "Diagnosticering",
            ("da", "Panel text size") => "Paneltekststørrelse",
            ("da", "Panel item spacing") => "Afstand mellem panelelementer",
            ("da", "Refresh") => "Opdater",
            ("da", "Refresh now") => "Opdater nu",
            ("da", "Load devices") => "Indlæs enheder",
            ("da", "Search devices") => "Søg efter enheder",
            ("da", "No devices match your search") => "Ingen enheder matcher søgningen",
            ("da", "Temperature") => "Temperatur",
            ("da", "Average temperature") => "Gennemsnitstemperatur",
            ("da", "Wind speed") => "Vindhastighed",
            ("da", "Direction") => "Retning",
            ("da", "Pressure") => "Lufttryk",
            ("da", "Humidity") => "Luftfugtighed",
            ("da", "Clouds") => "Skyer",
            ("da", "Visibility") => "Sigtbarhed",
            ("da", "Sunrise") => "Solopgang",
            ("da", "Sunset") => "Solnedgang",
            ("da", "Feels like") => "Føles som",
            ("da", "Now") => "Nu",
            ("da", "DOWNLOAD") => "DOWNLOAD",
            ("da", "UPLOAD") => "UPLOAD",
            ("da", "POWER (W)") => "EFFEKT (W)",
            ("da", "SETTINGS") => "INDSTILLINGER",
            ("da", "QUICK SETTINGS") => "HURTIGE INDSTILLINGER",
            ("da", "Connection error") => "Forbindelsesfejl",
            ("da", "Connection failed") => "Forbindelsen mislykkedes",
            ("da", "Settings saved") => "Indstillinger gemt",
            ("da", "Enabled") => "Aktiveret",
            ("da", "Disabled") => "Deaktiveret",
            ("da", "On") => "Til",
            ("da", "Off") => "Fra",
            ("da", "Upper limit") => "Øvre grænse",
            ("da", "Lower limit") => "Nedre grænse",
            ("da", "Temperature alerts") => "Temperaturadvarsler",
            ("da", "Graph") => "Diagram",
            ("da", "Line") => "Linje",
            ("da", "Bar") => "Søjler",
            ("da", "Forecast unavailable") => "Vejrudsigt ikke tilgængelig",
            ("da", "Loading forecast…") => "Indlæser vejrudsigt…",

            // Norwegian Bokmål (also used for generic Norwegian locale)
            ("no", "Settings") => "Innstillinger",
            ("no", "Configure your power monitor") => "Konfigurer strømmåleren",
            ("no", "GENERAL") => "GENERELT",
            ("no", "Language") => "Språk",
            ("no", "Interface language") => "Grensesnittspråk",
            ("no", "System default") => "Systemspråk",
            ("no", "English") => "Engelsk",
            ("no", "Deutsch") => "Tysk",
            ("no", "Panel") => "Panel",
            ("no", "Graph history") => "Diagramhistorikk",
            ("no", "Automatic refresh") => "Automatisk oppdatering",
            ("no", "Weather") => "Vær",
            ("no", "Connection") => "Tilkobling",
            ("no", "Devices") => "Enheter",
            ("no", "Menu order") => "Menyrekkefølge",
            ("no", "Panel order") => "Panelrekkefølge",
            ("no", "Diagnostics") => "Diagnostikk",
            ("no", "Panel text size") => "Paneltekststørrelse",
            ("no", "Panel item spacing") => "Avstand mellom panelelementer",
            ("no", "Refresh") => "Oppdater",
            ("no", "Refresh now") => "Oppdater nå",
            ("no", "Load devices") => "Last inn enheter",
            ("no", "Search devices") => "Søk etter enheter",
            ("no", "No devices match your search") => "Ingen enheter samsvarer med søket",
            ("no", "Temperature") => "Temperatur",
            ("no", "Average temperature") => "Gjennomsnittstemperatur",
            ("no", "Wind speed") => "Vindhastighet",
            ("no", "Direction") => "Retning",
            ("no", "Pressure") => "Lufttrykk",
            ("no", "Humidity") => "Luftfuktighet",
            ("no", "Clouds") => "Skydekke",
            ("no", "Visibility") => "Sikt",
            ("no", "Sunrise") => "Soloppgang",
            ("no", "Sunset") => "Solnedgang",
            ("no", "Feels like") => "Føles som",
            ("no", "Now") => "Nå",
            ("no", "DOWNLOAD") => "NEDLASTING",
            ("no", "UPLOAD") => "OPPLASTING",
            ("no", "POWER (W)") => "EFFEKT (W)",
            ("no", "SETTINGS") => "INNSTILLINGER",
            ("no", "QUICK SETTINGS") => "HURTIGINNSTILLINGER",
            ("no", "Connection error") => "Tilkoblingsfeil",
            ("no", "Connection failed") => "Tilkoblingen mislyktes",
            ("no", "Settings saved") => "Innstillingene er lagret",
            ("no", "Enabled") => "Aktivert",
            ("no", "Disabled") => "Deaktivert",
            ("no", "On") => "På",
            ("no", "Off") => "Av",
            ("no", "Upper limit") => "Øvre grense",
            ("no", "Lower limit") => "Nedre grense",
            ("no", "Temperature alerts") => "Temperaturvarsler",
            ("no", "Graph") => "Diagram",
            ("no", "Line") => "Linje",
            ("no", "Bar") => "Søyler",
            ("no", "Forecast unavailable") => "Værvarsel utilgjengelig",
            ("no", "Loading forecast…") => "Laster værvarsel…",

            // Expanded es interface and diagnostics coverage
            ("es", "Close") => "Cerrar",
            ("es", "Panel item order") => "Orden de los elementos del panel",
            ("es", "Adjusts the power and temperature text in the panel") => {
                "Ajusta el texto de potencia y temperatura del panel"
            }
            ("es", "Controls the gap between panel items") => {
                "Controla el espacio entre los elementos del panel"
            }
            ("es", "Multiline panel values") => "Valores del panel en varias líneas",
            ("es", "Stack related readings vertically instead of placing them on one line.") => {
                "Muestra las lecturas relacionadas en vertical en lugar de en una sola línea."
            }
            ("es", "Show panel refresh button") => "Mostrar botón de actualización en el panel",
            ("es", "Display a separate refresh button beside the applet in the panel.") => {
                "Muestra un botón de actualización independiente junto al applet en el panel."
            }
            ("es", "Show internet bandwidth") => "Mostrar el ancho de banda de Internet",
            ("es", "Combine consumption and solar power") => "Combinar consumo y energía solar",
            ("es", "Show one net power value: total consumption minus solar generation.") => {
                "Muestra una única potencia neta: consumo total menos generación solar."
            }
            ("es", "Exclude solar devices") => "Excluir dispositivos solares",
            (
                "es",
                "Ignore temperatures from devices whose name contains “solar” when calculating the average.",
            ) => {
                "Ignora las temperaturas de los dispositivos cuyo nombre contiene «solar» al calcular la media."
            }
            ("es", "GRAPH STYLE") => "ESTILO DE GRÁFICA",
            ("es", "30 seconds to 1 hour") => "De 30 segundos a 1 hora",
            ("es", "Use current weather icon") => "Usar el icono del tiempo actual",
            ("es", "OpenWeatherMap API key") => "Clave de API de OpenWeatherMap",
            ("es", "API key") => "Clave de API",
            ("es", "Location (city or latitude,longitude)") => {
                "Ubicación (ciudad o latitud,longitud)"
            }
            ("es", "Weather refresh") => "Actualización del tiempo",
            ("es", "Slideshow image duration") => "Duración de las imágenes",
            ("es", "FRITZ!Box URL") => "URL de FRITZ!Box",
            ("es", "Username") => "Nombre de usuario",
            ("es", "Password") => "Contraseña",
            ("es", "Allow invalid TLS certificates") => "Permitir certificados TLS no válidos",
            ("es", "Popup element order") => "Orden de los elementos de la ventana",
            ("es", "Move up") => "Mover arriba",
            ("es", "Move down") => "Mover abajo",
            ("es", "Select power-meter devices") => {
                "Selecciona los dispositivos medidores de energía"
            }
            ("es", "Input device (separate total)") => {
                "Dispositivo de entrada (total independiente)"
            }
            ("es", "Connection security") => "Seguridad de la conexión",
            ("es", "External resources") => "Servicios externos",
            ("es", "System notifications") => "Notificaciones del sistema",
            ("es", "Send test notification") => "Enviar notificación de prueba",
            ("es", "Latest request succeeded") => "La última solicitud se completó correctamente",
            ("es", "Latest request failed") => "La última solicitud falló",
            ("es", "Not checked yet") => "Aún no comprobado",
            ("es", "Get 7-day forecast") => "Obtener pronóstico de 7 días",
            ("es", "HTTP successful") => "HTTP correcto",
            ("es", "HTTP failed") => "Error HTTP",
            ("es", "Connected · device data loaded") => {
                "Conectado · datos de dispositivos cargados"
            }
            ("es", "Request in progress") => "Solicitud en curso",
            ("es", "No additional status messages") => "No hay más mensajes de estado",
            ("es", "Photo") => "Foto",
            ("es", "Forecast data by Open-Meteo") => "Pronóstico de Open-Meteo",
            ("es", "DOWNLOAD HISTORY") => "HISTORIAL DE DESCARGA",
            ("es", "UPLOAD HISTORY") => "HISTORIAL DE SUBIDA",
            ("es", "Select one or more devices in Settings") => {
                "Selecciona uno o más dispositivos en Ajustes"
            }
            ("es", "Fritz Power Monitor notification test") => {
                "Prueba de notificaciones de Fritz Power Monitor"
            }
            ("es", "Notifications are working correctly.") => {
                "Las notificaciones funcionan correctamente."
            }

            // Expanded nl interface and diagnostics coverage
            ("nl", "Close") => "Sluiten",
            ("nl", "Panel item order") => "Volgorde van paneelitems",
            ("nl", "Adjusts the power and temperature text in the panel") => {
                "Past de tekstgrootte van vermogen en temperatuur in het paneel aan"
            }
            ("nl", "Controls the gap between panel items") => "Regelt de ruimte tussen paneelitems",
            ("nl", "Multiline panel values") => "Paneelwaarden op meerdere regels",
            ("nl", "Stack related readings vertically instead of placing them on one line.") => {
                "Toont bij elkaar horende metingen onder elkaar in plaats van op één regel."
            }
            ("nl", "Show panel refresh button") => "Vernieuwknop in paneel tonen",
            ("nl", "Display a separate refresh button beside the applet in the panel.") => {
                "Toont een aparte vernieuwknop naast de applet in het paneel."
            }
            ("nl", "Show internet bandwidth") => "Internetbandbreedte tonen",
            ("nl", "Combine consumption and solar power") => "Verbruik en zonne-energie combineren",
            ("nl", "Show one net power value: total consumption minus solar generation.") => {
                "Toont één nettovermogenswaarde: totaal verbruik min zonne-opwekking."
            }
            ("nl", "Exclude solar devices") => "Zonnepanelen uitsluiten",
            (
                "nl",
                "Ignore temperatures from devices whose name contains “solar” when calculating the average.",
            ) => {
                "Negeert temperaturen van apparaten met ‘solar’ in de naam bij het berekenen van het gemiddelde."
            }
            ("nl", "GRAPH STYLE") => "GRAFIEKSTIJL",
            ("nl", "30 seconds to 1 hour") => "30 seconden tot 1 uur",
            ("nl", "Use current weather icon") => "Huidig weerpictogram gebruiken",
            ("nl", "OpenWeatherMap API key") => "OpenWeatherMap API-sleutel",
            ("nl", "API key") => "API-sleutel",
            ("nl", "Location (city or latitude,longitude)") => {
                "Locatie (plaats of breedtegraad,lengtegraad)"
            }
            ("nl", "Weather refresh") => "Weer vernieuwen",
            ("nl", "Slideshow image duration") => "Duur van diavoorstelling",
            ("nl", "FRITZ!Box URL") => "FRITZ!Box-URL",
            ("nl", "Username") => "Gebruikersnaam",
            ("nl", "Password") => "Wachtwoord",
            ("nl", "Allow invalid TLS certificates") => "Ongeldige TLS-certificaten toestaan",
            ("nl", "Popup element order") => "Volgorde van pop-upitems",
            ("nl", "Move up") => "Omhoog verplaatsen",
            ("nl", "Move down") => "Omlaag verplaatsen",
            ("nl", "Select power-meter devices") => "Selecteer energiemeterapparaten",
            ("nl", "Input device (separate total)") => "Invoerapparaat (apart totaal)",
            ("nl", "Connection security") => "Verbindingsbeveiliging",
            ("nl", "External resources") => "Externe bronnen",
            ("nl", "System notifications") => "Systeemmeldingen",
            ("nl", "Send test notification") => "Testmelding verzenden",
            ("nl", "Latest request succeeded") => "Laatste verzoek geslaagd",
            ("nl", "Latest request failed") => "Laatste verzoek mislukt",
            ("nl", "Not checked yet") => "Nog niet gecontroleerd",
            ("nl", "Get 7-day forecast") => "7-daagse verwachting ophalen",
            ("nl", "HTTP successful") => "HTTP geslaagd",
            ("nl", "HTTP failed") => "HTTP mislukt",
            ("nl", "Connected · device data loaded") => "Verbonden · apparaatgegevens geladen",
            ("nl", "Request in progress") => "Verzoek wordt uitgevoerd",
            ("nl", "No additional status messages") => "Geen aanvullende statusmeldingen",
            ("nl", "Photo") => "Foto",
            ("nl", "Forecast data by Open-Meteo") => "Verwachting door Open-Meteo",
            ("nl", "DOWNLOAD HISTORY") => "DOWNLOADGESCHIEDENIS",
            ("nl", "UPLOAD HISTORY") => "UPLOADGESCHIEDENIS",
            ("nl", "Select one or more devices in Settings") => {
                "Selecteer een of meer apparaten in Instellingen"
            }
            ("nl", "Fritz Power Monitor notification test") => {
                "Meldingstest van Fritz Power Monitor"
            }
            ("nl", "Notifications are working correctly.") => "Meldingen werken correct.",

            // Expanded fr interface and diagnostics coverage
            ("fr", "Close") => "Fermer",
            ("fr", "Panel item order") => "Ordre des éléments du panneau",
            ("fr", "Adjusts the power and temperature text in the panel") => {
                "Règle la taille du texte de puissance et de température dans le panneau"
            }
            ("fr", "Controls the gap between panel items") => {
                "Règle l’espacement entre les éléments du panneau"
            }
            ("fr", "Multiline panel values") => "Valeurs du panneau sur plusieurs lignes",
            ("fr", "Stack related readings vertically instead of placing them on one line.") => {
                "Affiche les mesures associées verticalement plutôt que sur une seule ligne."
            }
            ("fr", "Show panel refresh button") => "Afficher le bouton d’actualisation du panneau",
            ("fr", "Display a separate refresh button beside the applet in the panel.") => {
                "Affiche un bouton d’actualisation distinct à côté de l’applet dans le panneau."
            }
            ("fr", "Show internet bandwidth") => "Afficher le débit Internet",
            ("fr", "Combine consumption and solar power") => {
                "Combiner consommation et énergie solaire"
            }
            ("fr", "Show one net power value: total consumption minus solar generation.") => {
                "Affiche une puissance nette : consommation totale moins production solaire."
            }
            ("fr", "Exclude solar devices") => "Exclure les appareils solaires",
            (
                "fr",
                "Ignore temperatures from devices whose name contains “solar” when calculating the average.",
            ) => {
                "Ignore les températures des appareils dont le nom contient « solar » lors du calcul de la moyenne."
            }
            ("fr", "GRAPH STYLE") => "STYLE DU GRAPHIQUE",
            ("fr", "30 seconds to 1 hour") => "De 30 secondes à 1 heure",
            ("fr", "Use current weather icon") => "Utiliser l’icône météo actuelle",
            ("fr", "OpenWeatherMap API key") => "Clé API OpenWeatherMap",
            ("fr", "API key") => "Clé API",
            ("fr", "Location (city or latitude,longitude)") => "Lieu (ville ou latitude,longitude)",
            ("fr", "Weather refresh") => "Actualisation météo",
            ("fr", "Slideshow image duration") => "Durée d’affichage des images",
            ("fr", "FRITZ!Box URL") => "URL de la FRITZ!Box",
            ("fr", "Username") => "Nom d’utilisateur",
            ("fr", "Password") => "Mot de passe",
            ("fr", "Allow invalid TLS certificates") => "Autoriser les certificats TLS non valides",
            ("fr", "Popup element order") => "Ordre des éléments de la fenêtre",
            ("fr", "Move up") => "Monter",
            ("fr", "Move down") => "Descendre",
            ("fr", "Select power-meter devices") => {
                "Sélectionner les appareils de mesure électrique"
            }
            ("fr", "Input device (separate total)") => "Appareil d’entrée (total séparé)",
            ("fr", "Connection security") => "Sécurité de la connexion",
            ("fr", "External resources") => "Services externes",
            ("fr", "System notifications") => "Notifications système",
            ("fr", "Send test notification") => "Envoyer une notification de test",
            ("fr", "Latest request succeeded") => "Dernière requête réussie",
            ("fr", "Latest request failed") => "Dernière requête échouée",
            ("fr", "Not checked yet") => "Pas encore vérifié",
            ("fr", "Get 7-day forecast") => "Obtenir les prévisions sur 7 jours",
            ("fr", "HTTP successful") => "HTTP réussi",
            ("fr", "HTTP failed") => "Échec HTTP",
            ("fr", "Connected · device data loaded") => {
                "Connecté · données des appareils chargées"
            }
            ("fr", "Request in progress") => "Requête en cours",
            ("fr", "No additional status messages") => "Aucun autre message d’état",
            ("fr", "Photo") => "Photo",
            ("fr", "Forecast data by Open-Meteo") => "Prévisions par Open-Meteo",
            ("fr", "DOWNLOAD HISTORY") => "HISTORIQUE DES TÉLÉCHARGEMENTS",
            ("fr", "UPLOAD HISTORY") => "HISTORIQUE DES ENVOIS",
            ("fr", "Select one or more devices in Settings") => {
                "Sélectionnez un ou plusieurs appareils dans les paramètres"
            }
            ("fr", "Fritz Power Monitor notification test") => {
                "Test de notification Fritz Power Monitor"
            }
            ("fr", "Notifications are working correctly.") => {
                "Les notifications fonctionnent correctement."
            }

            // Expanded it interface and diagnostics coverage
            ("it", "Close") => "Chiudi",
            ("it", "Panel item order") => "Ordine degli elementi del pannello",
            ("it", "Adjusts the power and temperature text in the panel") => {
                "Regola la dimensione del testo di potenza e temperatura nel pannello"
            }
            ("it", "Controls the gap between panel items") => {
                "Regola la distanza tra gli elementi del pannello"
            }
            ("it", "Multiline panel values") => "Valori del pannello su più righe",
            ("it", "Stack related readings vertically instead of placing them on one line.") => {
                "Dispone le letture correlate in verticale anziché su un'unica riga."
            }
            ("it", "Show panel refresh button") => {
                "Mostra il pulsante di aggiornamento nel pannello"
            }
            ("it", "Display a separate refresh button beside the applet in the panel.") => {
                "Mostra un pulsante di aggiornamento separato accanto all'applet nel pannello."
            }
            ("it", "Show internet bandwidth") => "Mostra la larghezza di banda Internet",
            ("it", "Combine consumption and solar power") => "Combina consumo ed energia solare",
            ("it", "Show one net power value: total consumption minus solar generation.") => {
                "Mostra un unico valore netto: consumo totale meno produzione solare."
            }
            ("it", "Exclude solar devices") => "Escludi i dispositivi solari",
            (
                "it",
                "Ignore temperatures from devices whose name contains “solar” when calculating the average.",
            ) => {
                "Ignora le temperature dei dispositivi il cui nome contiene «solar» nel calcolo della media."
            }
            ("it", "GRAPH STYLE") => "STILE DEL GRAFICO",
            ("it", "30 seconds to 1 hour") => "Da 30 secondi a 1 ora",
            ("it", "Use current weather icon") => "Usa l'icona meteo attuale",
            ("it", "OpenWeatherMap API key") => "Chiave API OpenWeatherMap",
            ("it", "API key") => "Chiave API",
            ("it", "Location (city or latitude,longitude)") => {
                "Posizione (città o latitudine,longitudine)"
            }
            ("it", "Weather refresh") => "Aggiornamento meteo",
            ("it", "Slideshow image duration") => "Durata delle immagini",
            ("it", "FRITZ!Box URL") => "URL FRITZ!Box",
            ("it", "Username") => "Nome utente",
            ("it", "Password") => "Password",
            ("it", "Allow invalid TLS certificates") => "Consenti certificati TLS non validi",
            ("it", "Popup element order") => "Ordine degli elementi del popup",
            ("it", "Move up") => "Sposta in alto",
            ("it", "Move down") => "Sposta in basso",
            ("it", "Select power-meter devices") => "Seleziona i dispositivi di misurazione",
            ("it", "Input device (separate total)") => "Dispositivo di ingresso (totale separato)",
            ("it", "Connection security") => "Sicurezza della connessione",
            ("it", "External resources") => "Risorse esterne",
            ("it", "System notifications") => "Notifiche di sistema",
            ("it", "Send test notification") => "Invia notifica di prova",
            ("it", "Latest request succeeded") => "Ultima richiesta riuscita",
            ("it", "Latest request failed") => "Ultima richiesta non riuscita",
            ("it", "Not checked yet") => "Non ancora verificato",
            ("it", "Get 7-day forecast") => "Ottieni previsioni a 7 giorni",
            ("it", "HTTP successful") => "HTTP riuscito",
            ("it", "HTTP failed") => "HTTP non riuscito",
            ("it", "Connected · device data loaded") => "Connesso · dati dei dispositivi caricati",
            ("it", "Request in progress") => "Richiesta in corso",
            ("it", "No additional status messages") => "Nessun altro messaggio di stato",
            ("it", "Photo") => "Foto",
            ("it", "Forecast data by Open-Meteo") => "Previsioni fornite da Open-Meteo",
            ("it", "DOWNLOAD HISTORY") => "CRONOLOGIA DOWNLOAD",
            ("it", "UPLOAD HISTORY") => "CRONOLOGIA UPLOAD",
            ("it", "Select one or more devices in Settings") => {
                "Seleziona uno o più dispositivi nelle impostazioni"
            }
            ("it", "Fritz Power Monitor notification test") => "Test notifiche Fritz Power Monitor",
            ("it", "Notifications are working correctly.") => {
                "Le notifiche funzionano correttamente."
            }

            // Expanded sv interface and diagnostics coverage
            ("sv", "Close") => "Stäng",
            ("sv", "Panel item order") => "Ordning för panelobjekt",
            ("sv", "Adjusts the power and temperature text in the panel") => {
                "Justerar textstorleken för effekt och temperatur i panelen"
            }
            ("sv", "Controls the gap between panel items") => "Styr avståndet mellan panelobjekten",
            ("sv", "Multiline panel values") => "Panelvärden på flera rader",
            ("sv", "Stack related readings vertically instead of placing them on one line.") => {
                "Visa relaterade mätvärden vertikalt i stället för på en rad."
            }
            ("sv", "Show panel refresh button") => "Visa uppdateringsknapp i panelen",
            ("sv", "Display a separate refresh button beside the applet in the panel.") => {
                "Visar en separat uppdateringsknapp bredvid panelappen."
            }
            ("sv", "Show internet bandwidth") => "Visa internethastighet",
            ("sv", "Combine consumption and solar power") => "Kombinera förbrukning och solel",
            ("sv", "Show one net power value: total consumption minus solar generation.") => {
                "Visa ett nettovärde: total förbrukning minus solelproduktion."
            }
            ("sv", "Exclude solar devices") => "Uteslut solenergienheter",
            (
                "sv",
                "Ignore temperatures from devices whose name contains “solar” when calculating the average.",
            ) => {
                "Ignorera temperaturer från enheter vars namn innehåller ”solar” när medelvärdet beräknas."
            }
            ("sv", "GRAPH STYLE") => "GRAFSTIL",
            ("sv", "30 seconds to 1 hour") => "30 sekunder till 1 timme",
            ("sv", "Use current weather icon") => "Använd aktuell väderikon",
            ("sv", "OpenWeatherMap API key") => "OpenWeatherMap API-nyckel",
            ("sv", "API key") => "API-nyckel",
            ("sv", "Location (city or latitude,longitude)") => "Plats (ort eller latitud,longitud)",
            ("sv", "Weather refresh") => "Väderuppdatering",
            ("sv", "Slideshow image duration") => "Bildvisningstid",
            ("sv", "FRITZ!Box URL") => "FRITZ!Box-URL",
            ("sv", "Username") => "Användarnamn",
            ("sv", "Password") => "Lösenord",
            ("sv", "Allow invalid TLS certificates") => "Tillåt ogiltiga TLS-certifikat",
            ("sv", "Popup element order") => "Ordning för popupobjekt",
            ("sv", "Move up") => "Flytta upp",
            ("sv", "Move down") => "Flytta ned",
            ("sv", "Select power-meter devices") => "Välj elmätarenheter",
            ("sv", "Input device (separate total)") => "Inmatningsenhet (separat total)",
            ("sv", "Connection security") => "Anslutningssäkerhet",
            ("sv", "External resources") => "Externa resurser",
            ("sv", "System notifications") => "Systemaviseringar",
            ("sv", "Send test notification") => "Skicka testavisering",
            ("sv", "Latest request succeeded") => "Senaste begäran lyckades",
            ("sv", "Latest request failed") => "Senaste begäran misslyckades",
            ("sv", "Not checked yet") => "Inte kontrollerad ännu",
            ("sv", "Get 7-day forecast") => "Hämta 7-dagarsprognos",
            ("sv", "HTTP successful") => "HTTP lyckades",
            ("sv", "HTTP failed") => "HTTP misslyckades",
            ("sv", "Connected · device data loaded") => "Ansluten · enhetsdata har lästs in",
            ("sv", "Request in progress") => "Begäran pågår",
            ("sv", "No additional status messages") => "Inga ytterligare statusmeddelanden",
            ("sv", "Photo") => "Foto",
            ("sv", "Forecast data by Open-Meteo") => "Prognosdata från Open-Meteo",
            ("sv", "DOWNLOAD HISTORY") => "NEDLADDNINGSHISTORIK",
            ("sv", "UPLOAD HISTORY") => "UPPLADDNINGSHISTORIK",
            ("sv", "Select one or more devices in Settings") => {
                "Välj en eller flera enheter i Inställningar"
            }
            ("sv", "Fritz Power Monitor notification test") => {
                "Aviseringstest för Fritz Power Monitor"
            }
            ("sv", "Notifications are working correctly.") => "Aviseringarna fungerar.",

            // Expanded pt interface and diagnostics coverage
            ("pt", "Close") => "Fechar",
            ("pt", "Panel item order") => "Ordem dos elementos do painel",
            ("pt", "Adjusts the power and temperature text in the panel") => {
                "Ajusta o tamanho do texto da potência e temperatura no painel"
            }
            ("pt", "Controls the gap between panel items") => {
                "Controla o espaço entre os elementos do painel"
            }
            ("pt", "Multiline panel values") => "Valores do painel em várias linhas",
            ("pt", "Stack related readings vertically instead of placing them on one line.") => {
                "Apresenta as leituras relacionadas na vertical em vez de numa só linha."
            }
            ("pt", "Show panel refresh button") => "Mostrar botão de atualização no painel",
            ("pt", "Display a separate refresh button beside the applet in the panel.") => {
                "Mostra um botão de atualização separado junto à aplicação no painel."
            }
            ("pt", "Show internet bandwidth") => "Mostrar largura de banda da Internet",
            ("pt", "Combine consumption and solar power") => "Combinar consumo e energia solar",
            ("pt", "Show one net power value: total consumption minus solar generation.") => {
                "Mostra um valor líquido: consumo total menos produção solar."
            }
            ("pt", "Exclude solar devices") => "Excluir dispositivos solares",
            (
                "pt",
                "Ignore temperatures from devices whose name contains “solar” when calculating the average.",
            ) => {
                "Ignora as temperaturas de dispositivos cujo nome contém «solar» ao calcular a média."
            }
            ("pt", "GRAPH STYLE") => "ESTILO DO GRÁFICO",
            ("pt", "30 seconds to 1 hour") => "30 segundos a 1 hora",
            ("pt", "Use current weather icon") => "Usar o ícone meteorológico atual",
            ("pt", "OpenWeatherMap API key") => "Chave de API OpenWeatherMap",
            ("pt", "API key") => "Chave de API",
            ("pt", "Location (city or latitude,longitude)") => {
                "Localização (cidade ou latitude,longitude)"
            }
            ("pt", "Weather refresh") => "Atualização meteorológica",
            ("pt", "Slideshow image duration") => "Duração das imagens",
            ("pt", "FRITZ!Box URL") => "URL da FRITZ!Box",
            ("pt", "Username") => "Nome de utilizador",
            ("pt", "Password") => "Palavra-passe",
            ("pt", "Allow invalid TLS certificates") => "Permitir certificados TLS inválidos",
            ("pt", "Popup element order") => "Ordem dos elementos da janela",
            ("pt", "Move up") => "Mover para cima",
            ("pt", "Move down") => "Mover para baixo",
            ("pt", "Select power-meter devices") => "Selecionar dispositivos de medição de energia",
            ("pt", "Input device (separate total)") => "Dispositivo de entrada (total separado)",
            ("pt", "Connection security") => "Segurança da ligação",
            ("pt", "External resources") => "Recursos externos",
            ("pt", "System notifications") => "Notificações do sistema",
            ("pt", "Send test notification") => "Enviar notificação de teste",
            ("pt", "Latest request succeeded") => "Último pedido concluído com êxito",
            ("pt", "Latest request failed") => "Último pedido falhou",
            ("pt", "Not checked yet") => "Ainda não verificado",
            ("pt", "Get 7-day forecast") => "Obter previsão de 7 dias",
            ("pt", "HTTP successful") => "HTTP bem-sucedido",
            ("pt", "HTTP failed") => "Falha HTTP",
            ("pt", "Connected · device data loaded") => {
                "Ligado · dados dos dispositivos carregados"
            }
            ("pt", "Request in progress") => "Pedido em curso",
            ("pt", "No additional status messages") => "Sem mensagens de estado adicionais",
            ("pt", "Photo") => "Fotografia",
            ("pt", "Forecast data by Open-Meteo") => "Previsão fornecida por Open-Meteo",
            ("pt", "DOWNLOAD HISTORY") => "HISTÓRICO DE TRANSFERÊNCIAS",
            ("pt", "UPLOAD HISTORY") => "HISTÓRICO DE ENVIOS",
            ("pt", "Select one or more devices in Settings") => {
                "Selecione um ou mais dispositivos nas definições"
            }
            ("pt", "Fritz Power Monitor notification test") => {
                "Teste de notificações do Fritz Power Monitor"
            }
            ("pt", "Notifications are working correctly.") => {
                "As notificações estão a funcionar corretamente."
            }

            // Expanded pl interface and diagnostics coverage
            ("pl", "Close") => "Zamknij",
            ("pl", "Panel item order") => "Kolejność elementów panelu",
            ("pl", "Adjusts the power and temperature text in the panel") => {
                "Zmienia rozmiar tekstu mocy i temperatury w panelu"
            }
            ("pl", "Controls the gap between panel items") => {
                "Ustawia odstęp między elementami panelu"
            }
            ("pl", "Multiline panel values") => "Wartości panelu w wielu wierszach",
            ("pl", "Stack related readings vertically instead of placing them on one line.") => {
                "Wyświetla powiązane odczyty pionowo zamiast w jednym wierszu."
            }
            ("pl", "Show panel refresh button") => "Pokaż przycisk odświeżania w panelu",
            ("pl", "Display a separate refresh button beside the applet in the panel.") => {
                "Wyświetla osobny przycisk odświeżania obok apletu w panelu."
            }
            ("pl", "Show internet bandwidth") => "Pokaż przepustowość Internetu",
            ("pl", "Combine consumption and solar power") => "Połącz zużycie i energię słoneczną",
            ("pl", "Show one net power value: total consumption minus solar generation.") => {
                "Pokazuje jedną wartość netto: całkowite zużycie minus produkcja energii słonecznej."
            }
            ("pl", "Exclude solar devices") => "Wyklucz urządzenia solarne",
            (
                "pl",
                "Ignore temperatures from devices whose name contains “solar” when calculating the average.",
            ) => {
                "Pomija temperatury urządzeń, których nazwa zawiera „solar”, podczas obliczania średniej."
            }
            ("pl", "GRAPH STYLE") => "STYL WYKRESU",
            ("pl", "30 seconds to 1 hour") => "Od 30 sekund do 1 godziny",
            ("pl", "Use current weather icon") => "Użyj ikony bieżącej pogody",
            ("pl", "OpenWeatherMap API key") => "Klucz API OpenWeatherMap",
            ("pl", "API key") => "Klucz API",
            ("pl", "Location (city or latitude,longitude)") => {
                "Lokalizacja (miasto lub szerokość,długość geograficzna)"
            }
            ("pl", "Weather refresh") => "Odświeżanie pogody",
            ("pl", "Slideshow image duration") => "Czas wyświetlania zdjęcia",
            ("pl", "FRITZ!Box URL") => "Adres URL FRITZ!Box",
            ("pl", "Username") => "Nazwa użytkownika",
            ("pl", "Password") => "Hasło",
            ("pl", "Allow invalid TLS certificates") => "Zezwalaj na nieprawidłowe certyfikaty TLS",
            ("pl", "Popup element order") => "Kolejność elementów okna",
            ("pl", "Move up") => "Przenieś w górę",
            ("pl", "Move down") => "Przenieś w dół",
            ("pl", "Select power-meter devices") => "Wybierz urządzenia pomiarowe",
            ("pl", "Input device (separate total)") => "Urządzenie wejściowe (osobna suma)",
            ("pl", "Connection security") => "Bezpieczeństwo połączenia",
            ("pl", "External resources") => "Zasoby zewnętrzne",
            ("pl", "System notifications") => "Powiadomienia systemowe",
            ("pl", "Send test notification") => "Wyślij powiadomienie testowe",
            ("pl", "Latest request succeeded") => "Ostatnie żądanie zakończone powodzeniem",
            ("pl", "Latest request failed") => "Ostatnie żądanie nie powiodło się",
            ("pl", "Not checked yet") => "Jeszcze nie sprawdzono",
            ("pl", "Get 7-day forecast") => "Pobierz prognozę na 7 dni",
            ("pl", "HTTP successful") => "Żądanie HTTP powiodło się",
            ("pl", "HTTP failed") => "Żądanie HTTP nie powiodło się",
            ("pl", "Connected · device data loaded") => "Połączono · wczytano dane urządzeń",
            ("pl", "Request in progress") => "Trwa żądanie",
            ("pl", "No additional status messages") => "Brak dodatkowych komunikatów o stanie",
            ("pl", "Photo") => "Zdjęcie",
            ("pl", "Forecast data by Open-Meteo") => "Prognoza z Open-Meteo",
            ("pl", "DOWNLOAD HISTORY") => "HISTORIA POBIERANIA",
            ("pl", "UPLOAD HISTORY") => "HISTORIA WYSYŁANIA",
            ("pl", "Select one or more devices in Settings") => {
                "Wybierz co najmniej jedno urządzenie w ustawieniach"
            }
            ("pl", "Fritz Power Monitor notification test") => {
                "Test powiadomień Fritz Power Monitor"
            }
            ("pl", "Notifications are working correctly.") => "Powiadomienia działają prawidłowo.",

            // Expanded da interface and diagnostics coverage
            ("da", "Close") => "Luk",
            ("da", "Panel item order") => "Rækkefølge for panelelementer",
            ("da", "Adjusts the power and temperature text in the panel") => {
                "Justerer tekststørrelsen for effekt og temperatur i panelet"
            }
            ("da", "Controls the gap between panel items") => {
                "Styrer afstanden mellem panelelementer"
            }
            ("da", "Multiline panel values") => "Panelværdier på flere linjer",
            ("da", "Stack related readings vertically instead of placing them on one line.") => {
                "Viser relaterede målinger lodret i stedet for på én linje."
            }
            ("da", "Show panel refresh button") => "Vis opdateringsknap i panelet",
            ("da", "Display a separate refresh button beside the applet in the panel.") => {
                "Viser en separat opdateringsknap ved siden af appletten i panelet."
            }
            ("da", "Show internet bandwidth") => "Vis internetbåndbredde",
            ("da", "Combine consumption and solar power") => "Kombinér forbrug og solenergi",
            ("da", "Show one net power value: total consumption minus solar generation.") => {
                "Vis én nettoværdi: samlet forbrug minus solproduktion."
            }
            ("da", "Exclude solar devices") => "Udeluk solenergienheder",
            (
                "da",
                "Ignore temperatures from devices whose name contains “solar” when calculating the average.",
            ) => {
                "Ignorer temperaturer fra enheder med ”solar” i navnet ved beregning af gennemsnittet."
            }
            ("da", "GRAPH STYLE") => "GRAFSTIL",
            ("da", "30 seconds to 1 hour") => "30 sekunder til 1 time",
            ("da", "Use current weather icon") => "Brug aktuelt vejrikon",
            ("da", "OpenWeatherMap API key") => "OpenWeatherMap API-nøgle",
            ("da", "API key") => "API-nøgle",
            ("da", "Location (city or latitude,longitude)") => {
                "Placering (by eller breddegrad,længdegrad)"
            }
            ("da", "Weather refresh") => "Vejropdatering",
            ("da", "Slideshow image duration") => "Varighed af billeder",
            ("da", "FRITZ!Box URL") => "FRITZ!Box-URL",
            ("da", "Username") => "Brugernavn",
            ("da", "Password") => "Adgangskode",
            ("da", "Allow invalid TLS certificates") => "Tillad ugyldige TLS-certifikater",
            ("da", "Popup element order") => "Rækkefølge for popupelementer",
            ("da", "Move up") => "Flyt op",
            ("da", "Move down") => "Flyt ned",
            ("da", "Select power-meter devices") => "Vælg elmålerenheder",
            ("da", "Input device (separate total)") => "Inputenhed (separat total)",
            ("da", "Connection security") => "Forbindelsessikkerhed",
            ("da", "External resources") => "Eksterne ressourcer",
            ("da", "System notifications") => "Systemmeddelelser",
            ("da", "Send test notification") => "Send testmeddelelse",
            ("da", "Latest request succeeded") => "Seneste anmodning lykkedes",
            ("da", "Latest request failed") => "Seneste anmodning mislykkedes",
            ("da", "Not checked yet") => "Ikke kontrolleret endnu",
            ("da", "Get 7-day forecast") => "Hent 7-dagesprognose",
            ("da", "HTTP successful") => "HTTP lykkedes",
            ("da", "HTTP failed") => "HTTP mislykkedes",
            ("da", "Connected · device data loaded") => "Forbundet · enhedsdata indlæst",
            ("da", "Request in progress") => "Anmodning i gang",
            ("da", "No additional status messages") => "Ingen yderligere statusmeddelelser",
            ("da", "Photo") => "Foto",
            ("da", "Forecast data by Open-Meteo") => "Prognose fra Open-Meteo",
            ("da", "DOWNLOAD HISTORY") => "DOWNLOADHISTORIK",
            ("da", "UPLOAD HISTORY") => "UPLOADHISTORIK",
            ("da", "Select one or more devices in Settings") => {
                "Vælg en eller flere enheder i Indstillinger"
            }
            ("da", "Fritz Power Monitor notification test") => {
                "Fritz Power Monitor-meddelelsestest"
            }
            ("da", "Notifications are working correctly.") => "Meddelelser fungerer korrekt.",

            // Expanded no interface and diagnostics coverage
            ("no", "Close") => "Lukk",
            ("no", "Panel item order") => "Rekkefølge for panelelementer",
            ("no", "Adjusts the power and temperature text in the panel") => {
                "Justerer tekststørrelsen for effekt og temperatur i panelet"
            }
            ("no", "Controls the gap between panel items") => {
                "Styrer avstanden mellom panelelementene"
            }
            ("no", "Multiline panel values") => "Panelverdier over flere linjer",
            ("no", "Stack related readings vertically instead of placing them on one line.") => {
                "Viser relaterte målinger vertikalt i stedet for på én linje."
            }
            ("no", "Show panel refresh button") => "Vis oppdateringsknapp i panelet",
            ("no", "Display a separate refresh button beside the applet in the panel.") => {
                "Viser en egen oppdateringsknapp ved siden av appleten i panelet."
            }
            ("no", "Show internet bandwidth") => "Vis internetthastighet",
            ("no", "Combine consumption and solar power") => "Kombiner forbruk og solenergi",
            ("no", "Show one net power value: total consumption minus solar generation.") => {
                "Vis én nettoverdi: totalt forbruk minus solproduksjon."
            }
            ("no", "Exclude solar devices") => "Utelat solenergienheter",
            (
                "no",
                "Ignore temperatures from devices whose name contains “solar” when calculating the average.",
            ) => {
                "Ignorer temperaturer fra enheter med «solar» i navnet når gjennomsnittet beregnes."
            }
            ("no", "GRAPH STYLE") => "GRAFSTIL",
            ("no", "30 seconds to 1 hour") => "30 sekunder til 1 time",
            ("no", "Use current weather icon") => "Bruk gjeldende værikon",
            ("no", "OpenWeatherMap API key") => "OpenWeatherMap API-nøkkel",
            ("no", "API key") => "API-nøkkel",
            ("no", "Location (city or latitude,longitude)") => {
                "Sted (by eller breddegrad, lengdegrad)"
            }
            ("no", "Weather refresh") => "Væroppdatering",
            ("no", "Slideshow image duration") => "Varighet for lysbildefremvisning",
            ("no", "FRITZ!Box URL") => "FRITZ!Box-URL",
            ("no", "Username") => "Brukernavn",
            ("no", "Password") => "Passord",
            ("no", "Allow invalid TLS certificates") => "Tillat ugyldige TLS-sertifikater",
            ("no", "Popup element order") => "Rekkefølge for popup-elementer",
            ("no", "Move up") => "Flytt opp",
            ("no", "Move down") => "Flytt ned",
            ("no", "Select power-meter devices") => "Velg strømmålerenheter",
            ("no", "Input device (separate total)") => "Inndataenhet (egen totalsum)",
            ("no", "Connection security") => "Tilkoblingssikkerhet",
            ("no", "External resources") => "Eksterne ressurser",
            ("no", "System notifications") => "Systemvarsler",
            ("no", "Send test notification") => "Send testvarsel",
            ("no", "Latest request succeeded") => "Siste forespørsel lyktes",
            ("no", "Latest request failed") => "Siste forespørsel mislyktes",
            ("no", "Not checked yet") => "Ikke kontrollert ennå",
            ("no", "Get 7-day forecast") => "Hent 7-dagersvarsel",
            ("no", "HTTP successful") => "HTTP lyktes",
            ("no", "HTTP failed") => "HTTP mislyktes",
            ("no", "Connected · device data loaded") => "Tilkoblet · enhetsdata lastet inn",
            ("no", "Request in progress") => "Forespørsel pågår",
            ("no", "No additional status messages") => "Ingen flere statusmeldinger",
            ("no", "Photo") => "Bilde",
            ("no", "Forecast data by Open-Meteo") => "Værvarsel fra Open-Meteo",
            ("no", "DOWNLOAD HISTORY") => "NEDLASTINGSHISTORIKK",
            ("no", "UPLOAD HISTORY") => "OPPLASTINGSHISTORIKK",
            ("no", "Select one or more devices in Settings") => {
                "Velg én eller flere enheter i Innstillinger"
            }
            ("no", "Fritz Power Monitor notification test") => {
                "Varslingstest for Fritz Power Monitor"
            }
            ("no", "Notifications are working correctly.") => "Varslingene fungerer som de skal.",

            // Additional es labels for graphs, TLS and service diagnostics
            ("es", "High temperature") => "Temperatura alta",
            ("es", "Low temperature") => "Temperatura baja",
            ("es", "maximum") => "máximo",
            ("es", "minimum") => "mínimo",
            ("es", "Open graph") => "Abrir gráfica",
            ("es", "CURRENT POWER HISTORY") => "HISTORIAL DE POTENCIA ACTUAL",
            ("es", "CURRENT POWER") => "POTENCIA ACTUAL",
            ("es", "AVERAGE TEMPERATURE HISTORY (°C)") => "HISTORIAL DE TEMPERATURA MEDIA (°C)",
            ("es", "AVERAGE TEMPERATURE") => "TEMPERATURA MEDIA",
            ("es", "GRAPH HISTORY") => "HISTORIAL DE GRÁFICAS",
            ("es", "Move device up") => "Mover dispositivo arriba",
            ("es", "Move device down") => "Mover dispositivo abajo",
            ("es", "Certificate validation disabled") => "Validación de certificados desactivada",
            ("es", "Custom CA certificate configured") => {
                "Certificado CA personalizado configurado"
            }
            ("es", "System certificate roots") => "Certificados raíz del sistema",
            ("es", "Plain HTTP · TLS is not enabled") => "HTTP simple · TLS no está habilitado",
            ("es", "URL is invalid or not configured") => {
                "La URL no es válida o no está configurada"
            }
            ("es", "HTTPS / TLS") => "HTTPS / TLS",
            ("es", "Not configured") => "No configurado",
            ("es", "Send a test notification to verify desktop notification delivery.") => {
                "Envía una notificación de prueba para comprobar las notificaciones del escritorio."
            }
            ("es", "Image loaded") => "Imagen cargada",
            ("es", "No usable image loaded") => "No se ha cargado ninguna imagen utilizable",
            ("es", "Weather request failed") => "Error en la solicitud del tiempo",
            ("es", "Weather location not configured") => "Ubicación meteorológica no configurada",
            ("es", "Forecast request in progress") => "Solicitud de pronóstico en curso",
            ("es", "Forecast loaded") => "Pronóstico cargado",
            ("es", "Not checked · request a forecast to test") => {
                "No comprobado · solicita un pronóstico para probar"
            }
            ("es", "Request a forecast to verify the Open-Meteo endpoints.") => {
                "Solicita un pronóstico para verificar los servicios de Open-Meteo."
            }
            ("es", "Base URL not configured") => "URL base no configurada",
            ("es", "Session authentication") => "Autenticación de sesión",
            ("es", "Device list and meter readings") => {
                "Lista de dispositivos y lecturas de los medidores"
            }
            ("es", "Per-device switch state") => "Estado del interruptor por dispositivo",
            ("es", "Configured devices") => "Dispositivos configurados",
            ("es", "WEATHER UNAVAILABLE") => "TIEMPO NO DISPONIBLE",
            (
                "es",
                "This is normal when no errors or configuration changes need reporting. The endpoint states above show whether each service has been checked.",
            ) => {
                "Esto es normal cuando no hay errores ni cambios de configuración que comunicar. Los estados de los servicios indican si se han comprobado."
            }
            (
                "es",
                "Use Reload now to test the FRITZ!Box connection, Get 7-day forecast to test Open-Meteo, or Send test notification to verify desktop notifications.",
            ) => {
                "Usa Recargar ahora para probar la conexión FRITZ!Box, Obtener pronóstico de 7 días para probar Open-Meteo o Enviar notificación de prueba para comprobar las notificaciones."
            }
            ("es", "FORECAST UNAVAILABLE") => "PRONÓSTICO NO DISPONIBLE",
            ("es", "Weather data is loading…") => "Cargando datos meteorológicos…",
            ("es", "TEMPERATURE (°C)") => "TEMPERATURA (°C)",
            ("es", "INTERNET BANDWIDTH") => "ANCHO DE BANDA DE INTERNET",
            ("es", "CONNECTION ERROR") => "ERROR DE CONEXIÓN",

            // Additional nl labels for graphs, TLS and service diagnostics
            ("nl", "High temperature") => "Hoge temperatuur",
            ("nl", "Low temperature") => "Lage temperatuur",
            ("nl", "maximum") => "maximum",
            ("nl", "minimum") => "minimum",
            ("nl", "Open graph") => "Grafiek openen",
            ("nl", "CURRENT POWER HISTORY") => "GESCHIEDENIS HUIDIG VERMOGEN",
            ("nl", "CURRENT POWER") => "HUIDIG VERMOGEN",
            ("nl", "AVERAGE TEMPERATURE HISTORY (°C)") => {
                "GESCHIEDENIS GEMIDDELDE TEMPERATUUR (°C)"
            }
            ("nl", "AVERAGE TEMPERATURE") => "GEMIDDELDE TEMPERATUUR",
            ("nl", "GRAPH HISTORY") => "GRAFIEKGESCHIEDENIS",
            ("nl", "Move device up") => "Apparaat omhoog verplaatsen",
            ("nl", "Move device down") => "Apparaat omlaag verplaatsen",
            ("nl", "Certificate validation disabled") => "Certificaatvalidatie uitgeschakeld",
            ("nl", "Custom CA certificate configured") => "Aangepast CA-certificaat ingesteld",
            ("nl", "System certificate roots") => "Systeemhoofdcertificaten",
            ("nl", "Plain HTTP · TLS is not enabled") => "Gewone HTTP · TLS is niet ingeschakeld",
            ("nl", "URL is invalid or not configured") => "URL is ongeldig of niet ingesteld",
            ("nl", "HTTPS / TLS") => "HTTPS / TLS",
            ("nl", "Not configured") => "Niet ingesteld",
            ("nl", "Send a test notification to verify desktop notification delivery.") => {
                "Verstuur een testmelding om de bureaubladmeldingen te controleren."
            }
            ("nl", "Image loaded") => "Afbeelding geladen",
            ("nl", "No usable image loaded") => "Geen bruikbare afbeelding geladen",
            ("nl", "Weather request failed") => "Weerverzoek mislukt",
            ("nl", "Weather location not configured") => "Weerlocatie niet ingesteld",
            ("nl", "Forecast request in progress") => "Weerverwachting wordt opgevraagd",
            ("nl", "Forecast loaded") => "Verwachting geladen",
            ("nl", "Not checked · request a forecast to test") => {
                "Nog niet gecontroleerd · vraag een verwachting op om te testen"
            }
            ("nl", "Request a forecast to verify the Open-Meteo endpoints.") => {
                "Vraag een verwachting op om de Open-Meteo-eindpunten te controleren."
            }
            ("nl", "Base URL not configured") => "Basis-URL niet ingesteld",
            ("nl", "Session authentication") => "Sessieauthenticatie",
            ("nl", "Device list and meter readings") => "Apparatenlijst en meterstanden",
            ("nl", "Per-device switch state") => "Schakelstatus per apparaat",
            ("nl", "Configured devices") => "Ingestelde apparaten",
            ("nl", "WEATHER UNAVAILABLE") => "WEER NIET BESCHIKBAAR",
            (
                "nl",
                "This is normal when no errors or configuration changes need reporting. The endpoint states above show whether each service has been checked.",
            ) => {
                "Dit is normaal wanneer er geen fouten of configuratiewijzigingen te melden zijn. De status van de eindpunten geeft aan of elke dienst is gecontroleerd."
            }
            (
                "nl",
                "Use Reload now to test the FRITZ!Box connection, Get 7-day forecast to test Open-Meteo, or Send test notification to verify desktop notifications.",
            ) => {
                "Gebruik Nu vernieuwen om de FRITZ!Box-verbinding te testen, haal de 7-daagse verwachting op om Open-Meteo te testen of verstuur een testmelding."
            }
            ("nl", "FORECAST UNAVAILABLE") => "VERWACHTING NIET BESCHIKBAAR",
            ("nl", "Weather data is loading…") => "Weergegevens worden geladen…",
            ("nl", "TEMPERATURE (°C)") => "TEMPERATUUR (°C)",
            ("nl", "INTERNET BANDWIDTH") => "INTERNETBANDBREEDTE",
            ("nl", "CONNECTION ERROR") => "VERBINDINGSFOUT",

            // Additional fr labels for graphs, TLS and service diagnostics
            ("fr", "High temperature") => "Température élevée",
            ("fr", "Low temperature") => "Température basse",
            ("fr", "maximum") => "maximum",
            ("fr", "minimum") => "minimum",
            ("fr", "Open graph") => "Ouvrir le graphique",
            ("fr", "CURRENT POWER HISTORY") => "HISTORIQUE DE LA PUISSANCE ACTUELLE",
            ("fr", "CURRENT POWER") => "PUISSANCE ACTUELLE",
            ("fr", "AVERAGE TEMPERATURE HISTORY (°C)") => {
                "HISTORIQUE DE LA TEMPÉRATURE MOYENNE (°C)"
            }
            ("fr", "AVERAGE TEMPERATURE") => "TEMPÉRATURE MOYENNE",
            ("fr", "GRAPH HISTORY") => "HISTORIQUE DES GRAPHIQUES",
            ("fr", "Move device up") => "Déplacer l’appareil vers le haut",
            ("fr", "Move device down") => "Déplacer l’appareil vers le bas",
            ("fr", "Certificate validation disabled") => "Validation des certificats désactivée",
            ("fr", "Custom CA certificate configured") => "Certificat CA personnalisé configuré",
            ("fr", "System certificate roots") => "Autorités de certification du système",
            ("fr", "Plain HTTP · TLS is not enabled") => "HTTP simple · TLS n’est pas activé",
            ("fr", "URL is invalid or not configured") => "URL invalide ou non configurée",
            ("fr", "HTTPS / TLS") => "HTTPS / TLS",
            ("fr", "Not configured") => "Non configuré",
            ("fr", "Send a test notification to verify desktop notification delivery.") => {
                "Envoyer une notification de test pour vérifier les notifications du bureau."
            }
            ("fr", "Image loaded") => "Image chargée",
            ("fr", "No usable image loaded") => "Aucune image utilisable chargée",
            ("fr", "Weather request failed") => "Échec de la requête météo",
            ("fr", "Weather location not configured") => "Lieu météo non configuré",
            ("fr", "Forecast request in progress") => "Requête de prévision en cours",
            ("fr", "Forecast loaded") => "Prévisions chargées",
            ("fr", "Not checked · request a forecast to test") => {
                "Non vérifié · demander des prévisions pour tester"
            }
            ("fr", "Request a forecast to verify the Open-Meteo endpoints.") => {
                "Demandez des prévisions pour vérifier les services Open-Meteo."
            }
            ("fr", "Base URL not configured") => "URL de base non configurée",
            ("fr", "Session authentication") => "Authentification de session",
            ("fr", "Device list and meter readings") => {
                "Liste des appareils et relevés des compteurs"
            }
            ("fr", "Per-device switch state") => "État de l’interrupteur par appareil",
            ("fr", "Configured devices") => "Appareils configurés",
            ("fr", "WEATHER UNAVAILABLE") => "MÉTÉO INDISPONIBLE",
            (
                "fr",
                "This is normal when no errors or configuration changes need reporting. The endpoint states above show whether each service has been checked.",
            ) => {
                "Ceci est normal lorsqu’aucune erreur ni modification de configuration ne doit être signalée. Les états des services indiquent si chacun a été vérifié."
            }
            (
                "fr",
                "Use Reload now to test the FRITZ!Box connection, Get 7-day forecast to test Open-Meteo, or Send test notification to verify desktop notifications.",
            ) => {
                "Utilisez Actualiser pour tester la connexion FRITZ!Box, demandez les prévisions sur 7 jours pour tester Open-Meteo ou envoyez une notification de test."
            }
            ("fr", "FORECAST UNAVAILABLE") => "PRÉVISIONS INDISPONIBLES",
            ("fr", "Weather data is loading…") => "Chargement des données météo…",
            ("fr", "TEMPERATURE (°C)") => "TEMPÉRATURE (°C)",
            ("fr", "INTERNET BANDWIDTH") => "DÉBIT INTERNET",
            ("fr", "CONNECTION ERROR") => "ERREUR DE CONNEXION",

            // Additional it labels for graphs, TLS and service diagnostics
            ("it", "High temperature") => "Temperatura elevata",
            ("it", "Low temperature") => "Temperatura bassa",
            ("it", "maximum") => "massimo",
            ("it", "minimum") => "minimo",
            ("it", "Open graph") => "Apri grafico",
            ("it", "CURRENT POWER HISTORY") => "CRONOLOGIA POTENZA ATTUALE",
            ("it", "CURRENT POWER") => "POTENZA ATTUALE",
            ("it", "AVERAGE TEMPERATURE HISTORY (°C)") => "CRONOLOGIA TEMPERATURA MEDIA (°C)",
            ("it", "AVERAGE TEMPERATURE") => "TEMPERATURA MEDIA",
            ("it", "GRAPH HISTORY") => "CRONOLOGIA GRAFICI",
            ("it", "Move device up") => "Sposta dispositivo in alto",
            ("it", "Move device down") => "Sposta dispositivo in basso",
            ("it", "Certificate validation disabled") => "Convalida dei certificati disattivata",
            ("it", "Custom CA certificate configured") => {
                "Certificato CA personalizzato configurato"
            }
            ("it", "System certificate roots") => "Certificati radice di sistema",
            ("it", "Plain HTTP · TLS is not enabled") => "HTTP semplice · TLS non è abilitato",
            ("it", "URL is invalid or not configured") => "URL non valido o non configurato",
            ("it", "HTTPS / TLS") => "HTTPS / TLS",
            ("it", "Not configured") => "Non configurato",
            ("it", "Send a test notification to verify desktop notification delivery.") => {
                "Invia una notifica di prova per verificare le notifiche del desktop."
            }
            ("it", "Image loaded") => "Immagine caricata",
            ("it", "No usable image loaded") => "Nessuna immagine utilizzabile caricata",
            ("it", "Weather request failed") => "Richiesta meteo non riuscita",
            ("it", "Weather location not configured") => "Località meteo non configurata",
            ("it", "Forecast request in progress") => "Richiesta previsioni in corso",
            ("it", "Forecast loaded") => "Previsioni caricate",
            ("it", "Not checked · request a forecast to test") => {
                "Non verificato · richiedi previsioni per provare"
            }
            ("it", "Request a forecast to verify the Open-Meteo endpoints.") => {
                "Richiedi previsioni per verificare i servizi Open-Meteo."
            }
            ("it", "Base URL not configured") => "URL di base non configurato",
            ("it", "Session authentication") => "Autenticazione della sessione",
            ("it", "Device list and meter readings") => {
                "Elenco dispositivi e letture dei contatori"
            }
            ("it", "Per-device switch state") => "Stato dell’interruttore per dispositivo",
            ("it", "Configured devices") => "Dispositivi configurati",
            ("it", "WEATHER UNAVAILABLE") => "METEO NON DISPONIBILE",
            (
                "it",
                "This is normal when no errors or configuration changes need reporting. The endpoint states above show whether each service has been checked.",
            ) => {
                "È normale quando non ci sono errori o modifiche alla configurazione da segnalare. Gli stati degli endpoint indicano se ogni servizio è stato verificato."
            }
            (
                "it",
                "Use Reload now to test the FRITZ!Box connection, Get 7-day forecast to test Open-Meteo, or Send test notification to verify desktop notifications.",
            ) => {
                "Usa Aggiorna ora per verificare la connessione FRITZ!Box, richiedi le previsioni a 7 giorni per provare Open-Meteo oppure invia una notifica di prova."
            }
            ("it", "FORECAST UNAVAILABLE") => "PREVISIONI NON DISPONIBILI",
            ("it", "Weather data is loading…") => "Caricamento dei dati meteo…",
            ("it", "TEMPERATURE (°C)") => "TEMPERATURA (°C)",
            ("it", "INTERNET BANDWIDTH") => "BANDA INTERNET",
            ("it", "CONNECTION ERROR") => "ERRORE DI CONNESSIONE",

            // Additional sv labels for graphs, TLS and service diagnostics
            ("sv", "High temperature") => "Hög temperatur",
            ("sv", "Low temperature") => "Låg temperatur",
            ("sv", "maximum") => "maximalt",
            ("sv", "minimum") => "minimalt",
            ("sv", "Open graph") => "Öppna diagram",
            ("sv", "CURRENT POWER HISTORY") => "HISTORIK FÖR AKTUELL EFFEKT",
            ("sv", "CURRENT POWER") => "AKTUELL EFFEKT",
            ("sv", "AVERAGE TEMPERATURE HISTORY (°C)") => "HISTORIK FÖR MEDELTEMPERATUR (°C)",
            ("sv", "AVERAGE TEMPERATURE") => "MEDELTEMPERATUR",
            ("sv", "GRAPH HISTORY") => "GRAFHISTORIK",
            ("sv", "Move device up") => "Flytta enheten uppåt",
            ("sv", "Move device down") => "Flytta enheten nedåt",
            ("sv", "Certificate validation disabled") => "Certifikatkontroll inaktiverad",
            ("sv", "Custom CA certificate configured") => "Anpassat CA-certifikat konfigurerat",
            ("sv", "System certificate roots") => "Systemets rotcertifikat",
            ("sv", "Plain HTTP · TLS is not enabled") => "Vanlig HTTP · TLS är inte aktiverat",
            ("sv", "URL is invalid or not configured") => {
                "URL:en är ogiltig eller inte konfigurerad"
            }
            ("sv", "HTTPS / TLS") => "HTTPS / TLS",
            ("sv", "Not configured") => "Inte konfigurerad",
            ("sv", "Send a test notification to verify desktop notification delivery.") => {
                "Skicka en testavisering för att kontrollera skrivbordsaviseringar."
            }
            ("sv", "Image loaded") => "Bild inläst",
            ("sv", "No usable image loaded") => "Ingen användbar bild har lästs in",
            ("sv", "Weather request failed") => "Väderförfrågan misslyckades",
            ("sv", "Weather location not configured") => "Väderplats inte konfigurerad",
            ("sv", "Forecast request in progress") => "Väderprognosförfrågan pågår",
            ("sv", "Forecast loaded") => "Prognos inläst",
            ("sv", "Not checked · request a forecast to test") => {
                "Inte kontrollerad · hämta en prognos för att testa"
            }
            ("sv", "Request a forecast to verify the Open-Meteo endpoints.") => {
                "Hämta en prognos för att kontrollera Open-Meteo-tjänsterna."
            }
            ("sv", "Base URL not configured") => "Bas-URL inte konfigurerad",
            ("sv", "Session authentication") => "Sessionsautentisering",
            ("sv", "Device list and meter readings") => "Enhetslista och mätaravläsningar",
            ("sv", "Per-device switch state") => "Strömbrytarstatus per enhet",
            ("sv", "Configured devices") => "Konfigurerade enheter",
            ("sv", "WEATHER UNAVAILABLE") => "VÄDER EJ TILLGÄNGLIGT",
            (
                "sv",
                "This is normal when no errors or configuration changes need reporting. The endpoint states above show whether each service has been checked.",
            ) => {
                "Detta är normalt när inga fel eller konfigurationsändringar behöver rapporteras. Tjänststatusen visar om varje tjänst har kontrollerats."
            }
            (
                "sv",
                "Use Reload now to test the FRITZ!Box connection, Get 7-day forecast to test Open-Meteo, or Send test notification to verify desktop notifications.",
            ) => {
                "Använd Uppdatera nu för att testa FRITZ!Box-anslutningen, hämta 7-dagarsprognosen för att testa Open-Meteo eller skicka en testavisering."
            }
            ("sv", "FORECAST UNAVAILABLE") => "PROGNOS EJ TILLGÄNGLIG",
            ("sv", "Weather data is loading…") => "Väderdata läses in…",
            ("sv", "TEMPERATURE (°C)") => "TEMPERATUR (°C)",
            ("sv", "INTERNET BANDWIDTH") => "INTERNETBAND\u{AD}BREDD",
            ("sv", "CONNECTION ERROR") => "ANSLUTNINGSFEL",

            // Additional pt labels for graphs, TLS and service diagnostics
            ("pt", "High temperature") => "Temperatura elevada",
            ("pt", "Low temperature") => "Temperatura baixa",
            ("pt", "maximum") => "máximo",
            ("pt", "minimum") => "mínimo",
            ("pt", "Open graph") => "Abrir gráfico",
            ("pt", "CURRENT POWER HISTORY") => "HISTÓRICO DA POTÊNCIA ATUAL",
            ("pt", "CURRENT POWER") => "POTÊNCIA ATUAL",
            ("pt", "AVERAGE TEMPERATURE HISTORY (°C)") => "HISTÓRICO DA TEMPERATURA MÉDIA (°C)",
            ("pt", "AVERAGE TEMPERATURE") => "TEMPERATURA MÉDIA",
            ("pt", "GRAPH HISTORY") => "HISTÓRICO DOS GRÁFICOS",
            ("pt", "Move device up") => "Mover dispositivo para cima",
            ("pt", "Move device down") => "Mover dispositivo para baixo",
            ("pt", "Certificate validation disabled") => "Validação de certificados desativada",
            ("pt", "Custom CA certificate configured") => {
                "Certificado CA personalizado configurado"
            }
            ("pt", "System certificate roots") => "Certificados raiz do sistema",
            ("pt", "Plain HTTP · TLS is not enabled") => "HTTP simples · TLS não está ativo",
            ("pt", "URL is invalid or not configured") => "URL inválido ou não configurado",
            ("pt", "HTTPS / TLS") => "HTTPS / TLS",
            ("pt", "Not configured") => "Não configurado",
            ("pt", "Send a test notification to verify desktop notification delivery.") => {
                "Enviar uma notificação de teste para verificar as notificações do ambiente de trabalho."
            }
            ("pt", "Image loaded") => "Imagem carregada",
            ("pt", "No usable image loaded") => "Nenhuma imagem utilizável carregada",
            ("pt", "Weather request failed") => "Falha no pedido meteorológico",
            ("pt", "Weather location not configured") => {
                "Localização meteorológica não configurada"
            }
            ("pt", "Forecast request in progress") => "Pedido de previsão em curso",
            ("pt", "Forecast loaded") => "Previsão carregada",
            ("pt", "Not checked · request a forecast to test") => {
                "Não verificado · peça uma previsão para testar"
            }
            ("pt", "Request a forecast to verify the Open-Meteo endpoints.") => {
                "Peça uma previsão para verificar os serviços Open-Meteo."
            }
            ("pt", "Base URL not configured") => "URL base não configurado",
            ("pt", "Session authentication") => "Autenticação de sessão",
            ("pt", "Device list and meter readings") => {
                "Lista de dispositivos e leituras dos medidores"
            }
            ("pt", "Per-device switch state") => "Estado do interruptor por dispositivo",
            ("pt", "Configured devices") => "Dispositivos configurados",
            ("pt", "WEATHER UNAVAILABLE") => "METEOROLOGIA INDISPONÍVEL",
            (
                "pt",
                "This is normal when no errors or configuration changes need reporting. The endpoint states above show whether each service has been checked.",
            ) => {
                "Isto é normal quando não existem erros ou alterações de configuração a comunicar. Os estados dos serviços indicam se cada um foi verificado."
            }
            (
                "pt",
                "Use Reload now to test the FRITZ!Box connection, Get 7-day forecast to test Open-Meteo, or Send test notification to verify desktop notifications.",
            ) => {
                "Use Atualizar agora para testar a ligação FRITZ!Box, obtenha a previsão de 7 dias para testar o Open-Meteo ou envie uma notificação de teste."
            }
            ("pt", "FORECAST UNAVAILABLE") => "PREVISÃO INDISPONÍVEL",
            ("pt", "Weather data is loading…") => "A carregar dados meteorológicos…",
            ("pt", "TEMPERATURE (°C)") => "TEMPERATURA (°C)",
            ("pt", "INTERNET BANDWIDTH") => "LARGURA DE BANDA DA INTERNET",
            ("pt", "CONNECTION ERROR") => "ERRO DE LIGAÇÃO",

            // Additional pl labels for graphs, TLS and service diagnostics
            ("pl", "High temperature") => "Wysoka temperatura",
            ("pl", "Low temperature") => "Niska temperatura",
            ("pl", "maximum") => "maksimum",
            ("pl", "minimum") => "minimum",
            ("pl", "Open graph") => "Otwórz wykres",
            ("pl", "CURRENT POWER HISTORY") => "HISTORIA BIEŻĄCEJ MOCY",
            ("pl", "CURRENT POWER") => "BIEŻĄCA MOC",
            ("pl", "AVERAGE TEMPERATURE HISTORY (°C)") => "HISTORIA ŚREDNIEJ TEMPERATURY (°C)",
            ("pl", "AVERAGE TEMPERATURE") => "ŚREDNIA TEMPERATURA",
            ("pl", "GRAPH HISTORY") => "HISTORIA WYKRESÓW",
            ("pl", "Move device up") => "Przenieś urządzenie w górę",
            ("pl", "Move device down") => "Przenieś urządzenie w dół",
            ("pl", "Certificate validation disabled") => "Weryfikacja certyfikatów wyłączona",
            ("pl", "Custom CA certificate configured") => {
                "Skonfigurowano niestandardowy certyfikat CA"
            }
            ("pl", "System certificate roots") => "Systemowe główne urzędy certyfikacji",
            ("pl", "Plain HTTP · TLS is not enabled") => "Zwykły HTTP · TLS nie jest włączony",
            ("pl", "URL is invalid or not configured") => {
                "Nieprawidłowy lub nieskonfigurowany adres URL"
            }
            ("pl", "HTTPS / TLS") => "HTTPS / TLS",
            ("pl", "Not configured") => "Nie skonfigurowano",
            ("pl", "Send a test notification to verify desktop notification delivery.") => {
                "Wyślij powiadomienie testowe, aby sprawdzić powiadomienia pulpitu."
            }
            ("pl", "Image loaded") => "Obraz wczytany",
            ("pl", "No usable image loaded") => "Nie wczytano użytecznego obrazu",
            ("pl", "Weather request failed") => "Nieudane żądanie pogody",
            ("pl", "Weather location not configured") => "Nie skonfigurowano lokalizacji pogody",
            ("pl", "Forecast request in progress") => "Trwa pobieranie prognozy",
            ("pl", "Forecast loaded") => "Prognoza wczytana",
            ("pl", "Not checked · request a forecast to test") => {
                "Nie sprawdzono · pobierz prognozę, aby przetestować"
            }
            ("pl", "Request a forecast to verify the Open-Meteo endpoints.") => {
                "Pobierz prognozę, aby sprawdzić usługi Open-Meteo."
            }
            ("pl", "Base URL not configured") => "Nie skonfigurowano podstawowego adresu URL",
            ("pl", "Session authentication") => "Uwierzytelnianie sesji",
            ("pl", "Device list and meter readings") => "Lista urządzeń i odczyty liczników",
            ("pl", "Per-device switch state") => "Stan przełącznika urządzenia",
            ("pl", "Configured devices") => "Skonfigurowane urządzenia",
            ("pl", "WEATHER UNAVAILABLE") => "POGODA NIEDOSTĘPNA",
            (
                "pl",
                "This is normal when no errors or configuration changes need reporting. The endpoint states above show whether each service has been checked.",
            ) => {
                "To normalne, gdy nie ma błędów ani zmian konfiguracji do zgłoszenia. Stany usług pokazują, czy każda z nich została sprawdzona."
            }
            (
                "pl",
                "Use Reload now to test the FRITZ!Box connection, Get 7-day forecast to test Open-Meteo, or Send test notification to verify desktop notifications.",
            ) => {
                "Użyj opcji Odśwież teraz, aby sprawdzić połączenie FRITZ!Box, pobierz prognozę na 7 dni, aby sprawdzić Open-Meteo, lub wyślij powiadomienie testowe."
            }
            ("pl", "FORECAST UNAVAILABLE") => "PROGNOZA NIEDOSTĘPNA",
            ("pl", "Weather data is loading…") => "Trwa wczytywanie danych pogodowych…",
            ("pl", "TEMPERATURE (°C)") => "TEMPERATURA (°C)",
            ("pl", "INTERNET BANDWIDTH") => "PRZEPUSTOWOŚĆ INTERNETU",
            ("pl", "CONNECTION ERROR") => "BŁĄD POŁĄCZENIA",

            // Additional da labels for graphs, TLS and service diagnostics
            ("da", "High temperature") => "Høj temperatur",
            ("da", "Low temperature") => "Lav temperatur",
            ("da", "maximum") => "maksimum",
            ("da", "minimum") => "minimum",
            ("da", "Open graph") => "Åbn graf",
            ("da", "CURRENT POWER HISTORY") => "HISTORIK FOR AKTUEL EFFEKT",
            ("da", "CURRENT POWER") => "AKTUEL EFFEKT",
            ("da", "AVERAGE TEMPERATURE HISTORY (°C)") => {
                "HISTORIK FOR GENNEMSNITSTEMPERATUR (°C)"
            }
            ("da", "AVERAGE TEMPERATURE") => "GENNEMSNITSTEMPERATUR",
            ("da", "GRAPH HISTORY") => "GRAFHISTORIK",
            ("da", "Move device up") => "Flyt enhed op",
            ("da", "Move device down") => "Flyt enhed ned",
            ("da", "Certificate validation disabled") => "Certifikatvalidering deaktiveret",
            ("da", "Custom CA certificate configured") => {
                "Brugerdefineret CA-certifikat konfigureret"
            }
            ("da", "System certificate roots") => "Systemets rodcertifikater",
            ("da", "Plain HTTP · TLS is not enabled") => "Almindelig HTTP · TLS er ikke aktiveret",
            ("da", "URL is invalid or not configured") => "URL er ugyldig eller ikke konfigureret",
            ("da", "HTTPS / TLS") => "HTTPS / TLS",
            ("da", "Not configured") => "Ikke konfigureret",
            ("da", "Send a test notification to verify desktop notification delivery.") => {
                "Send en testmeddelelse for at kontrollere skrivebordsmeddelelser."
            }
            ("da", "Image loaded") => "Billede indlæst",
            ("da", "No usable image loaded") => "Intet brugbart billede indlæst",
            ("da", "Weather request failed") => "Vejranmodning mislykkedes",
            ("da", "Weather location not configured") => "Vejrplacering ikke konfigureret",
            ("da", "Forecast request in progress") => "Vejrudsigt anmodes",
            ("da", "Forecast loaded") => "Vejrudsigt indlæst",
            ("da", "Not checked · request a forecast to test") => {
                "Ikke kontrolleret · hent en vejrudsigt for at teste"
            }
            ("da", "Request a forecast to verify the Open-Meteo endpoints.") => {
                "Hent en vejrudsigt for at kontrollere Open-Meteo-tjenesterne."
            }
            ("da", "Base URL not configured") => "Basis-URL ikke konfigureret",
            ("da", "Session authentication") => "Sessionsgodkendelse",
            ("da", "Device list and meter readings") => "Enhedsliste og måleraflæsninger",
            ("da", "Per-device switch state") => "Afbryderstatus pr. enhed",
            ("da", "Configured devices") => "Konfigurerede enheder",
            ("da", "WEATHER UNAVAILABLE") => "VEJR IKKE TILGÆNGELIGT",
            (
                "da",
                "This is normal when no errors or configuration changes need reporting. The endpoint states above show whether each service has been checked.",
            ) => {
                "Dette er normalt, når der ikke er fejl eller konfigurationsændringer at rapportere. Tjenestestatus viser, om hver tjeneste er kontrolleret."
            }
            (
                "da",
                "Use Reload now to test the FRITZ!Box connection, Get 7-day forecast to test Open-Meteo, or Send test notification to verify desktop notifications.",
            ) => {
                "Brug Opdater nu til at teste FRITZ!Box-forbindelsen, hent 7-dagesprognosen for at teste Open-Meteo, eller send en testmeddelelse."
            }
            ("da", "FORECAST UNAVAILABLE") => "VEJRUDSIGT IKKE TILGÆNGELIG",
            ("da", "Weather data is loading…") => "Vejrdata indlæses…",
            ("da", "TEMPERATURE (°C)") => "TEMPERATUR (°C)",
            ("da", "INTERNET BANDWIDTH") => "INTERNETBÅNDBREDDE",
            ("da", "CONNECTION ERROR") => "FORBINDELSESFEJL",

            // Additional no labels for graphs, TLS and service diagnostics
            ("no", "High temperature") => "Høy temperatur",
            ("no", "Low temperature") => "Lav temperatur",
            ("no", "maximum") => "maksimum",
            ("no", "minimum") => "minimum",
            ("no", "Open graph") => "Åpne graf",
            ("no", "CURRENT POWER HISTORY") => "HISTORIKK FOR GJELDENDE EFFEKT",
            ("no", "CURRENT POWER") => "GJELDENDE EFFEKT",
            ("no", "AVERAGE TEMPERATURE HISTORY (°C)") => {
                "HISTORIKK FOR GJENNOMSNITTSTEMPERATUR (°C)"
            }
            ("no", "AVERAGE TEMPERATURE") => "GJENNOMSNITTSTEMPERATUR",
            ("no", "GRAPH HISTORY") => "GRAFHISTORIKK",
            ("no", "Move device up") => "Flytt enhet opp",
            ("no", "Move device down") => "Flytt enhet ned",
            ("no", "Certificate validation disabled") => "Sertifikatvalidering deaktivert",
            ("no", "Custom CA certificate configured") => "Egendefinert CA-sertifikat konfigurert",
            ("no", "System certificate roots") => "Systemets rotsertifikater",
            ("no", "Plain HTTP · TLS is not enabled") => "Vanlig HTTP · TLS er ikke aktivert",
            ("no", "URL is invalid or not configured") => "URL er ugyldig eller ikke konfigurert",
            ("no", "HTTPS / TLS") => "HTTPS / TLS",
            ("no", "Not configured") => "Ikke konfigurert",
            ("no", "Send a test notification to verify desktop notification delivery.") => {
                "Send et testvarsel for å kontrollere skrivebordsvarsler."
            }
            ("no", "Image loaded") => "Bilde lastet",
            ("no", "No usable image loaded") => "Ingen brukbare bilder lastet",
            ("no", "Weather request failed") => "Værforespørsel mislyktes",
            ("no", "Weather location not configured") => "Værplassering ikke konfigurert",
            ("no", "Forecast request in progress") => "Værvarselforespørsel pågår",
            ("no", "Forecast loaded") => "Værvarsel lastet",
            ("no", "Not checked · request a forecast to test") => {
                "Ikke kontrollert · hent et værvarsel for å teste"
            }
            ("no", "Request a forecast to verify the Open-Meteo endpoints.") => {
                "Hent et værvarsel for å kontrollere Open-Meteo-tjenestene."
            }
            ("no", "Base URL not configured") => "Basis-URL ikke konfigurert",
            ("no", "Session authentication") => "Sesjonsautentisering",
            ("no", "Device list and meter readings") => "Enhetsliste og måleravlesninger",
            ("no", "Per-device switch state") => "Bryterstatus per enhet",
            ("no", "Configured devices") => "Konfigurerte enheter",
            ("no", "WEATHER UNAVAILABLE") => "VÆR IKKE TILGJENGELIG",
            (
                "no",
                "This is normal when no errors or configuration changes need reporting. The endpoint states above show whether each service has been checked.",
            ) => {
                "Dette er normalt når det ikke finnes feil eller konfigurasjonsendringer som må rapporteres. Tjenestestatusen viser om hver tjeneste er kontrollert."
            }
            (
                "no",
                "Use Reload now to test the FRITZ!Box connection, Get 7-day forecast to test Open-Meteo, or Send test notification to verify desktop notifications.",
            ) => {
                "Bruk Oppdater nå for å teste FRITZ!Box-tilkoblingen, hent 7-dagersvarselet for å teste Open-Meteo, eller send et testvarsel."
            }
            ("no", "FORECAST UNAVAILABLE") => "VÆRVARSEL IKKE TILGJENGELIG",
            ("no", "Weather data is loading…") => "Laster værdata…",
            ("no", "TEMPERATURE (°C)") => "TEMPERATUR (°C)",
            ("no", "INTERNET BANDWIDTH") => "INTERNETTBÅNDBREDDE",
            ("no", "CONNECTION ERROR") => "TILKOBLINGSFEIL",

            // es device availability states
            ("es", "FRITZ!Box device unavailable") => "Dispositivo FRITZ!Box no disponible",
            ("es", "FRITZ!Box devices unavailable") => "Dispositivos FRITZ!Box no disponibles",

            // nl device availability states
            ("nl", "FRITZ!Box device unavailable") => "FRITZ!Box-apparaat niet beschikbaar",
            ("nl", "FRITZ!Box devices unavailable") => "FRITZ!Box-apparaten niet beschikbaar",

            // fr device availability states
            ("fr", "FRITZ!Box device unavailable") => "Appareil FRITZ!Box indisponible",
            ("fr", "FRITZ!Box devices unavailable") => "Appareils FRITZ!Box indisponibles",

            // it device availability states
            ("it", "FRITZ!Box device unavailable") => "Dispositivo FRITZ!Box non disponibile",
            ("it", "FRITZ!Box devices unavailable") => "Dispositivi FRITZ!Box non disponibili",

            // sv device availability states
            ("sv", "FRITZ!Box device unavailable") => "FRITZ!Box-enheten är inte tillgänglig",
            ("sv", "FRITZ!Box devices unavailable") => "FRITZ!Box-enheterna är inte tillgängliga",

            // pt device availability states
            ("pt", "FRITZ!Box device unavailable") => "Dispositivo FRITZ!Box indisponível",
            ("pt", "FRITZ!Box devices unavailable") => "Dispositivos FRITZ!Box indisponíveis",

            // pl device availability states
            ("pl", "FRITZ!Box device unavailable") => "Urządzenie FRITZ!Box jest niedostępne",
            ("pl", "FRITZ!Box devices unavailable") => "Urządzenia FRITZ!Box są niedostępne",

            // da device availability states
            ("da", "FRITZ!Box device unavailable") => "FRITZ!Box-enhed ikke tilgængelig",
            ("da", "FRITZ!Box devices unavailable") => "FRITZ!Box-enheder ikke tilgængelige",

            // no device availability states
            ("no", "FRITZ!Box device unavailable") => "FRITZ!Box-enhet er utilgjengelig",
            ("no", "FRITZ!Box devices unavailable") => "FRITZ!Box-enheter er utilgjengelige",

            // es weather setting descriptions
            ("es", "e.g. Amsterdam or 52.37,4.90") => "p. ej., Ámsterdam o 52.37,4.90",
            (
                "es",
                "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.",
            ) => {
                "Independiente de la actualización de FRITZ!Box; 10 minutos por defecto. Requiere una clave API y una ubicación."
            }
            (
                "es",
                "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition.",
            ) => {
                "Cada foto se muestra entre 5 segundos y 5 minutos antes de la transición de fundido."
            }

            // nl weather setting descriptions
            ("nl", "e.g. Amsterdam or 52.37,4.90") => "bijv. Amsterdam of 52.37,4.90",
            (
                "nl",
                "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.",
            ) => {
                "Onafhankelijk van FRITZ!Box-vernieuwing; standaard 10 minuten. Een API-sleutel en locatie zijn vereist."
            }
            (
                "nl",
                "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition.",
            ) => "Elke foto wordt 5 seconden tot 5 minuten getoond voordat de overgang begint.",

            // fr weather setting descriptions
            ("fr", "e.g. Amsterdam or 52.37,4.90") => "p. ex. Amsterdam ou 52.37,4.90",
            (
                "fr",
                "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.",
            ) => {
                "Indépendant de l’actualisation FRITZ!Box ; 10 minutes par défaut. Une clé API et un lieu sont requis."
            }
            (
                "fr",
                "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition.",
            ) => "Chaque photo s’affiche de 5 secondes à 5 minutes avant la transition en fondu.",

            // it weather setting descriptions
            ("it", "e.g. Amsterdam or 52.37,4.90") => "ad es. Amsterdam o 52.37,4.90",
            (
                "it",
                "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.",
            ) => {
                "Indipendente dall'aggiornamento FRITZ!Box; 10 minuti per impostazione predefinita. Richiede una chiave API e una posizione."
            }
            (
                "it",
                "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition.",
            ) => {
                "Ogni foto viene mostrata da 5 secondi a 5 minuti prima della transizione con dissolvenza."
            }

            // sv weather setting descriptions
            ("sv", "e.g. Amsterdam or 52.37,4.90") => "t.ex. Amsterdam eller 52.37,4.90",
            (
                "sv",
                "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.",
            ) => {
                "Oberoende av FRITZ!Box-uppdateringen; standard är 10 minuter. Kräver en API-nyckel och en plats."
            }
            (
                "sv",
                "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition.",
            ) => "Varje foto visas i 5 sekunder till 5 minuter före övertoningen.",

            // pt weather setting descriptions
            ("pt", "e.g. Amsterdam or 52.37,4.90") => "por ex., Amesterdão ou 52.37,4.90",
            (
                "pt",
                "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.",
            ) => {
                "Independente da atualização da FRITZ!Box; 10 minutos por predefinição. Requer uma chave de API e uma localização."
            }
            (
                "pt",
                "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition.",
            ) => {
                "Cada fotografia é apresentada durante 5 segundos a 5 minutos antes da transição gradual."
            }

            // pl weather setting descriptions
            ("pl", "e.g. Amsterdam or 52.37,4.90") => "np. Amsterdam lub 52.37,4.90",
            (
                "pl",
                "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.",
            ) => {
                "Niezależne od odświeżania FRITZ!Box; domyślnie 10 minut. Wymaga klucza API i lokalizacji."
            }
            (
                "pl",
                "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition.",
            ) => {
                "Każde zdjęcie jest wyświetlane od 5 sekund do 5 minut przed przejściem z przenikaniem."
            }

            // da weather setting descriptions
            ("da", "e.g. Amsterdam or 52.37,4.90") => "f.eks. Amsterdam eller 52.37,4.90",
            (
                "da",
                "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.",
            ) => {
                "Uafhængig af FRITZ!Box-opdatering; standard er 10 minutter. Kræver en API-nøgle og en placering."
            }
            (
                "da",
                "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition.",
            ) => "Hvert foto vises i 5 sekunder til 5 minutter før overtoningen.",

            // no weather setting descriptions
            ("no", "e.g. Amsterdam or 52.37,4.90") => "f.eks. Amsterdam eller 52.37,4.90",
            (
                "no",
                "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.",
            ) => {
                "Uavhengig av FRITZ!Box-oppdateringen; standard er 10 minutter. Krever en API-nøkkel og en plassering."
            }
            (
                "no",
                "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition.",
            ) => "Hvert bilde vises i 5 sekunder til 5 minutter før overgangen med toning.",

            ("en", "English") => "English",
            ("en", "German") => "German",
            ("en", "Spanish") => "Spanish",
            ("en", "Dutch") => "Dutch",
            ("en", "French") => "French",
            ("en", "Italian") => "Italian",
            ("en", "Swedish") => "Swedish",
            ("en", "Portuguese") => "Portuguese",
            ("en", "Polish") => "Polish",
            ("en", "Danish") => "Danish",
            ("en", "Norwegian") => "Norwegian",
            ("es", "German") => "Alemán",
            ("es", "Spanish") => "Español",
            ("es", "Dutch") => "Neerlandés",
            ("es", "French") => "Francés",
            ("es", "Italian") => "Italiano",
            ("es", "Swedish") => "Sueco",
            ("es", "Portuguese") => "Portugués",
            ("es", "Polish") => "Polaco",
            ("es", "Danish") => "Danés",
            ("es", "Norwegian") => "Noruego",
            ("nl", "German") => "Duits",
            ("nl", "Spanish") => "Spaans",
            ("nl", "Dutch") => "Nederlands",
            ("nl", "French") => "Frans",
            ("nl", "Italian") => "Italiaans",
            ("nl", "Swedish") => "Zweeds",
            ("nl", "Portuguese") => "Portugees",
            ("nl", "Polish") => "Pools",
            ("nl", "Danish") => "Deens",
            ("nl", "Norwegian") => "Noors",
            ("fr", "German") => "Allemand",
            ("fr", "Spanish") => "Espagnol",
            ("fr", "Dutch") => "Néerlandais",
            ("fr", "French") => "Français",
            ("fr", "Italian") => "Italien",
            ("fr", "Swedish") => "Suédois",
            ("fr", "Portuguese") => "Portugais",
            ("fr", "Polish") => "Polonais",
            ("fr", "Danish") => "Danois",
            ("fr", "Norwegian") => "Norvégien",
            ("it", "German") => "Tedesco",
            ("it", "Spanish") => "Spagnolo",
            ("it", "Dutch") => "Olandese",
            ("it", "French") => "Francese",
            ("it", "Italian") => "Italiano",
            ("it", "Swedish") => "Svedese",
            ("it", "Portuguese") => "Portoghese",
            ("it", "Polish") => "Polacco",
            ("it", "Danish") => "Danese",
            ("it", "Norwegian") => "Norvegese",
            ("sv", "German") => "Tyska",
            ("sv", "Spanish") => "Spanska",
            ("sv", "Dutch") => "Nederländska",
            ("sv", "French") => "Franska",
            ("sv", "Italian") => "Italienska",
            ("sv", "Swedish") => "Svenska",
            ("sv", "Portuguese") => "Portugisiska",
            ("sv", "Polish") => "Polska",
            ("sv", "Danish") => "Danska",
            ("sv", "Norwegian") => "Norska",
            ("pt", "German") => "Alemão",
            ("pt", "Spanish") => "Espanhol",
            ("pt", "Dutch") => "Neerlandês",
            ("pt", "French") => "Francês",
            ("pt", "Italian") => "Italiano",
            ("pt", "Swedish") => "Sueco",
            ("pt", "Portuguese") => "Português",
            ("pt", "Polish") => "Polaco",
            ("pt", "Danish") => "Dinamarquês",
            ("pt", "Norwegian") => "Norueguês",
            ("pl", "German") => "Niemiecki",
            ("pl", "Spanish") => "Hiszpański",
            ("pl", "Dutch") => "Niderlandzki",
            ("pl", "French") => "Francuski",
            ("pl", "Italian") => "Włoski",
            ("pl", "Swedish") => "Szwedzki",
            ("pl", "Portuguese") => "Portugalski",
            ("pl", "Polish") => "Polski",
            ("pl", "Danish") => "Duński",
            ("pl", "Norwegian") => "Norweski",
            ("da", "German") => "Tysk",
            ("da", "Spanish") => "Spansk",
            ("da", "Dutch") => "Hollandsk",
            ("da", "French") => "Fransk",
            ("da", "Italian") => "Italiensk",
            ("da", "Swedish") => "Svensk",
            ("da", "Portuguese") => "Portugisisk",
            ("da", "Polish") => "Polsk",
            ("da", "Danish") => "Dansk",
            ("da", "Norwegian") => "Norsk",
            ("no", "German") => "Tysk",
            ("no", "Spanish") => "Spansk",
            ("no", "Dutch") => "Nederlandsk",
            ("no", "French") => "Fransk",
            ("no", "Italian") => "Italiensk",
            ("no", "Swedish") => "Svensk",
            ("no", "Portuguese") => "Portugisisk",
            ("no", "Polish") => "Polsk",
            ("no", "Danish") => "Dansk",
            ("no", "Norwegian") => "Norsk",
            (
                "ja",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish and Norwegian.",
            ) => {
                "システムはデスクトップの言語に従います。利用可能な言語：英語、ドイツ語、スペイン語、オランダ語、フランス語、イタリア語、スウェーデン語、ポルトガル語、ポーランド語、デンマーク語、ノルウェー語、日本語、韓国語。"
            }
            (
                "ja",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean.",
            ) => {
                "システムはデスクトップの言語に従います。利用可能な言語：英語、ドイツ語、スペイン語、オランダ語、フランス語、イタリア語、スウェーデン語、ポルトガル語、ポーランド語、デンマーク語、ノルウェー語、日本語、韓国語。"
            }
            ("ja", "Settings") => "設定",
            ("ja", "Language") => "言語",
            ("ja", "Interface language") => "表示言語",
            ("ja", "System default") => "システムの既定",
            ("ja", "English") => "英語",
            ("ja", "German") => "ドイツ語",
            ("ja", "Spanish") => "スペイン語",
            ("ja", "Dutch") => "オランダ語",
            ("ja", "French") => "フランス語",
            ("ja", "Italian") => "イタリア語",
            ("ja", "Swedish") => "スウェーデン語",
            ("ja", "Portuguese") => "ポルトガル語",
            ("ja", "Polish") => "ポーランド語",
            ("ja", "Danish") => "デンマーク語",
            ("ja", "Norwegian") => "ノルウェー語",
            ("ja", "Japanese") => "日本語",
            ("ja", "Korean") => "韓国語",
            ("ja", "Panel") => "パネル",
            ("ja", "Graph history") => "グラフ履歴",
            ("ja", "Automatic refresh") => "自動更新",
            ("ja", "Weather") => "天気",
            ("ja", "Connection") => "接続",
            ("ja", "Devices") => "デバイス",
            ("ja", "Menu order") => "メニューの順序",
            ("ja", "Panel order") => "パネルの順序",
            ("ja", "Diagnostics") => "診断",
            ("ja", "Refresh") => "更新",
            ("ja", "Refresh now") => "今すぐ更新",
            ("ja", "Temperature") => "温度",
            ("ja", "Wind speed") => "風速",
            ("ja", "Direction") => "風向",
            ("ja", "Pressure") => "気圧",
            ("ja", "Humidity") => "湿度",
            ("ja", "Clouds") => "雲量",
            ("ja", "Visibility") => "視程",
            ("ja", "Sunrise") => "日の出",
            ("ja", "Sunset") => "日の入り",
            ("ja", "Feels like") => "体感温度",
            ("ja", "Now") => "現在",
            ("ja", "Connection error") => "接続エラー",
            ("ja", "Connection failed") => "接続に失敗しました",
            ("ja", "Settings saved") => "設定を保存しました",
            ("ja", "Enabled") => "有効",
            ("ja", "Disabled") => "無効",
            ("ja", "On") => "オン",
            ("ja", "Off") => "オフ",
            ("ja", "Upper limit") => "上限",
            ("ja", "Lower limit") => "下限",
            ("ja", "Temperature alerts") => "温度アラート",
            ("ja", "Graph") => "グラフ",
            ("ja", "Line") => "折れ線",
            ("ja", "Bar") => "棒",
            ("ja", "Forecast unavailable") => "予報を利用できません",
            ("ja", "Loading forecast…") => "予報を読み込み中…",
            ("ja", "Panel item order") => "パネル項目の順序",
            ("ja", "Show internet bandwidth") => "インターネット通信量を表示",
            ("ja", "FRITZ!Box URL") => "FRITZ!Box の URL",
            ("ja", "Connection security") => "接続セキュリティ",
            ("ja", "Send test notification") => "テスト通知を送信",
            ("ja", "Get 7-day forecast") => "7日間の予報を取得",
            ("ja", "Certificate validation disabled") => "証明書の検証は無効です",
            ("ja", "WEATHER UNAVAILABLE") => "天気情報を利用できません",
            ("ja", "FRITZ!Box device unavailable") => "FRITZ!Box デバイスを利用できません",
            ("ja", "Move device up") => "デバイスを上へ移動",
            ("ja", "Move device down") => "デバイスを下へ移動",
            ("ja", "High temperature") => "高温",
            ("ja", "Low temperature") => "低温",
            ("ja", "maximum") => "最大",
            ("ja", "minimum") => "最小",
            ("ja", "Close (Esc)") => "閉じる (Esc)",
            ("ja", "Open graph") => "グラフを開く",
            ("ja", "Save") => "保存",
            ("ja", "Cancel") => "キャンセル",
            ("ja", "Apply") => "適用",
            ("ja", "Location") => "場所",
            ("ja", "API key") => "APIキー",
            ("ja", "Current conditions") => "現在の天気",
            ("ja", "7-day forecast") => "7日間の予報",
            ("ja", "No data available") => "データがありません",
            ("ja", "Unknown") => "不明",
            ("ja", "Connected") => "接続済み",
            ("ja", "Not connected") => "未接続",
            ("ja", "Retry") => "再試行",
            ("ja", "Loading…") => "読み込み中…",
            ("ja", "Never") => "なし",
            ("ja", "Automatic") => "自動",
            ("ja", "Manual") => "手動",
            ("ja", "Select all") => "すべて選択",
            ("ja", "Deselect all") => "選択を解除",
            ("ja", "Configure your power monitor") => "電力モニターを設定",
            ("ja", "Panel text size") => "パネルの文字サイズ",
            ("ja", "Panel item spacing") => "パネル項目の間隔",
            ("ja", "No devices match your search") => "検索条件に一致するデバイスはありません",
            ("ja", "Average temperature") => "平均温度",
            ("ja", "DOWNLOAD") => "ダウンロード",
            ("ja", "UPLOAD") => "アップロード",
            ("ja", "POWER (W)") => "電力 (W)",
            ("ja", "SETTINGS") => "設定",
            ("ja", "QUICK SETTINGS") => "クイック設定",
            ("ja", "City") => "都市",
            ("ja", "Show") => "表示",
            ("ja", "Hide") => "非表示",
            (
                "ko",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish and Norwegian.",
            ) => {
                "시스템은 데스크톱 언어를 따릅니다. 사용 가능한 언어: 영어, 독일어, 스페인어, 네덜란드어, 프랑스어, 이탈리아어, 스웨덴어, 포르투갈어, 폴란드어, 덴마크어, 노르웨이어, 일본어, 한국어."
            }
            (
                "ko",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean.",
            ) => {
                "시스템은 데스크톱 언어를 따릅니다. 사용 가능한 언어: 영어, 독일어, 스페인어, 네덜란드어, 프랑스어, 이탈리아어, 스웨덴어, 포르투갈어, 폴란드어, 덴마크어, 노르웨이어, 일본어, 한국어."
            }
            ("ko", "Settings") => "설정",
            ("ko", "Language") => "언어",
            ("ko", "Interface language") => "인터페이스 언어",
            ("ko", "System default") => "시스템 기본값",
            ("ko", "English") => "영어",
            ("ko", "German") => "독일어",
            ("ko", "Spanish") => "스페인어",
            ("ko", "Dutch") => "네덜란드어",
            ("ko", "French") => "프랑스어",
            ("ko", "Italian") => "이탈리아어",
            ("ko", "Swedish") => "스웨덴어",
            ("ko", "Portuguese") => "포르투갈어",
            ("ko", "Polish") => "폴란드어",
            ("ko", "Danish") => "덴마크어",
            ("ko", "Norwegian") => "노르웨이어",
            ("ko", "Japanese") => "일본어",
            ("ko", "Korean") => "한국어",
            ("ko", "Panel") => "패널",
            ("ko", "Graph history") => "그래프 기록",
            ("ko", "Automatic refresh") => "자동 새로 고침",
            ("ko", "Weather") => "날씨",
            ("ko", "Connection") => "연결",
            ("ko", "Devices") => "장치",
            ("ko", "Menu order") => "메뉴 순서",
            ("ko", "Panel order") => "패널 순서",
            ("ko", "Diagnostics") => "진단",
            ("ko", "Refresh") => "새로 고침",
            ("ko", "Refresh now") => "지금 새로 고침",
            ("ko", "Temperature") => "온도",
            ("ko", "Wind speed") => "풍속",
            ("ko", "Direction") => "방향",
            ("ko", "Pressure") => "기압",
            ("ko", "Humidity") => "습도",
            ("ko", "Clouds") => "구름",
            ("ko", "Visibility") => "가시거리",
            ("ko", "Sunrise") => "일출",
            ("ko", "Sunset") => "일몰",
            ("ko", "Feels like") => "체감 온도",
            ("ko", "Now") => "현재",
            ("ko", "Connection error") => "연결 오류",
            ("ko", "Connection failed") => "연결 실패",
            ("ko", "Settings saved") => "설정을 저장했습니다",
            ("ko", "Enabled") => "사용",
            ("ko", "Disabled") => "사용 안 함",
            ("ko", "On") => "켜짐",
            ("ko", "Off") => "꺼짐",
            ("ko", "Upper limit") => "상한",
            ("ko", "Lower limit") => "하한",
            ("ko", "Temperature alerts") => "온도 알림",
            ("ko", "Graph") => "그래프",
            ("ko", "Line") => "선",
            ("ko", "Bar") => "막대",
            ("ko", "Forecast unavailable") => "예보를 사용할 수 없습니다",
            ("ko", "Loading forecast…") => "예보 불러오는 중…",
            ("ko", "Panel item order") => "패널 항목 순서",
            ("ko", "Show internet bandwidth") => "인터넷 대역폭 표시",
            ("ko", "FRITZ!Box URL") => "FRITZ!Box URL",
            ("ko", "Connection security") => "연결 보안",
            ("ko", "Send test notification") => "테스트 알림 보내기",
            ("ko", "Get 7-day forecast") => "7일 예보 가져오기",
            ("ko", "Certificate validation disabled") => "인증서 검증이 비활성화되었습니다",
            ("ko", "WEATHER UNAVAILABLE") => "날씨 정보를 사용할 수 없습니다",
            ("ko", "FRITZ!Box device unavailable") => "FRITZ!Box 장치를 사용할 수 없습니다",
            ("ko", "Move device up") => "장치 위로 이동",
            ("ko", "Move device down") => "장치 아래로 이동",
            ("ko", "High temperature") => "고온",
            ("ko", "Low temperature") => "저온",
            ("ko", "maximum") => "최대",
            ("ko", "minimum") => "최소",
            ("ko", "Close (Esc)") => "닫기 (Esc)",
            ("ko", "Open graph") => "그래프 열기",
            ("ko", "Save") => "저장",
            ("ko", "Cancel") => "취소",
            ("ko", "Apply") => "적용",
            ("ko", "Location") => "위치",
            ("ko", "API key") => "API 키",
            ("ko", "Current conditions") => "현재 날씨",
            ("ko", "7-day forecast") => "7일 예보",
            ("ko", "No data available") => "데이터가 없습니다",
            ("ko", "Unknown") => "알 수 없음",
            ("ko", "Connected") => "연결됨",
            ("ko", "Not connected") => "연결되지 않음",
            ("ko", "Retry") => "다시 시도",
            ("ko", "Loading…") => "불러오는 중…",
            ("ko", "Never") => "안 함",
            ("ko", "Automatic") => "자동",
            ("ko", "Manual") => "수동",
            ("ko", "Select all") => "모두 선택",
            ("ko", "Deselect all") => "선택 해제",
            ("ko", "Configure your power monitor") => "전력 모니터 설정",
            ("ko", "Panel text size") => "패널 글자 크기",
            ("ko", "Panel item spacing") => "패널 항목 간격",
            ("ko", "No devices match your search") => "검색 결과와 일치하는 장치가 없습니다",
            ("ko", "Average temperature") => "평균 온도",
            ("ko", "DOWNLOAD") => "다운로드",
            ("ko", "UPLOAD") => "업로드",
            ("ko", "POWER (W)") => "전력 (W)",
            ("ko", "SETTINGS") => "설정",
            ("ko", "QUICK SETTINGS") => "빠른 설정",
            ("ko", "City") => "도시",
            ("ko", "Show") => "표시",
            ("ko", "Hide") => "숨기기",
            ("es", "Japanese") => "Japonés",
            ("es", "Korean") => "Coreano",
            ("es", "Reload now") => "Recargar ahora",
            ("es", "TOTAL POWER INPUT") => "POTENCIA TOTAL DE ENTRADA",
            ("es", "TOTAL POWER OUTPUT") => "POTENCIA TOTAL DE SALIDA",
            ("es", "TOTAL ENERGY") => "ENERGÍA TOTAL",
            ("es", "Open-Meteo · forecast") => "Open-Meteo · pronóstico",
            ("es", "Open-Meteo · geocoding") => "Open-Meteo · geocodificación",
            ("es", "OpenWeatherMap · current weather") => "OpenWeatherMap · tiempo actual",
            ("es", "Wikimedia Commons · image search and download") => {
                "Wikimedia Commons · búsqueda y descarga de imágenes"
            }
            ("nl", "Japanese") => "Japans",
            ("nl", "Korean") => "Koreaans",
            ("nl", "Reload now") => "Nu opnieuw laden",
            ("nl", "TOTAL POWER INPUT") => "TOTAAL VERMOGEN IN",
            ("nl", "TOTAL POWER OUTPUT") => "TOTAAL VERMOGEN UIT",
            ("nl", "TOTAL ENERGY") => "TOTALE ENERGIE",
            ("nl", "Open-Meteo · forecast") => "Open-Meteo · verwachting",
            ("nl", "Open-Meteo · geocoding") => "Open-Meteo · geocodering",
            ("nl", "OpenWeatherMap · current weather") => "OpenWeatherMap · huidig weer",
            ("nl", "Wikimedia Commons · image search and download") => {
                "Wikimedia Commons · afbeeldingen zoeken en downloaden"
            }
            ("fr", "Japanese") => "Japonais",
            ("fr", "Korean") => "Coréen",
            ("fr", "Reload now") => "Recharger maintenant",
            ("fr", "TOTAL POWER INPUT") => "PUISSANCE TOTALE ENTRANTE",
            ("fr", "TOTAL POWER OUTPUT") => "PUISSANCE TOTALE SORTANTE",
            ("fr", "TOTAL ENERGY") => "ÉNERGIE TOTALE",
            ("fr", "Open-Meteo · forecast") => "Open-Meteo · prévisions",
            ("fr", "Open-Meteo · geocoding") => "Open-Meteo · géocodage",
            ("fr", "OpenWeatherMap · current weather") => "OpenWeatherMap · météo actuelle",
            ("fr", "Wikimedia Commons · image search and download") => {
                "Wikimedia Commons · recherche et téléchargement d’images"
            }
            ("it", "Japanese") => "Giapponese",
            ("it", "Korean") => "Coreano",
            ("it", "Reload now") => "Ricarica ora",
            ("it", "TOTAL POWER INPUT") => "POTENZA TOTALE IN INGRESSO",
            ("it", "TOTAL POWER OUTPUT") => "POTENZA TOTALE IN USCITA",
            ("it", "TOTAL ENERGY") => "ENERGIA TOTALE",
            ("it", "Open-Meteo · forecast") => "Open-Meteo · previsioni",
            ("it", "Open-Meteo · geocoding") => "Open-Meteo · geocodifica",
            ("it", "OpenWeatherMap · current weather") => "OpenWeatherMap · meteo attuale",
            ("it", "Wikimedia Commons · image search and download") => {
                "Wikimedia Commons · ricerca e download di immagini"
            }
            ("sv", "Japanese") => "Japanska",
            ("sv", "Korean") => "Koreanska",
            ("sv", "Reload now") => "Läs in igen",
            ("sv", "TOTAL POWER INPUT") => "TOTAL EFFEKT IN",
            ("sv", "TOTAL POWER OUTPUT") => "TOTAL EFFEKT UT",
            ("sv", "TOTAL ENERGY") => "TOTAL ENERGI",
            ("sv", "Open-Meteo · forecast") => "Open-Meteo · prognos",
            ("sv", "Open-Meteo · geocoding") => "Open-Meteo · geokodning",
            ("sv", "OpenWeatherMap · current weather") => "OpenWeatherMap · aktuellt väder",
            ("sv", "Wikimedia Commons · image search and download") => {
                "Wikimedia Commons · bildsökning och hämtning"
            }
            ("pt", "Japanese") => "Japonês",
            ("pt", "Korean") => "Coreano",
            ("pt", "Reload now") => "Recarregar agora",
            ("pt", "TOTAL POWER INPUT") => "POTÊNCIA TOTAL DE ENTRADA",
            ("pt", "TOTAL POWER OUTPUT") => "POTÊNCIA TOTAL DE SAÍDA",
            ("pt", "TOTAL ENERGY") => "ENERGIA TOTAL",
            ("pt", "Open-Meteo · forecast") => "Open-Meteo · previsão",
            ("pt", "Open-Meteo · geocoding") => "Open-Meteo · geocodificação",
            ("pt", "OpenWeatherMap · current weather") => "OpenWeatherMap · meteorologia atual",
            ("pt", "Wikimedia Commons · image search and download") => {
                "Wikimedia Commons · pesquisa e transferência de imagens"
            }
            ("pl", "Japanese") => "Japoński",
            ("pl", "Korean") => "Koreański",
            ("pl", "Reload now") => "Wczytaj ponownie",
            ("pl", "TOTAL POWER INPUT") => "CAŁKOWITA MOC WEJŚCIOWA",
            ("pl", "TOTAL POWER OUTPUT") => "CAŁKOWITA MOC WYJŚCIOWA",
            ("pl", "TOTAL ENERGY") => "CAŁKOWITA ENERGIA",
            ("pl", "Open-Meteo · forecast") => "Open-Meteo · prognoza",
            ("pl", "Open-Meteo · geocoding") => "Open-Meteo · geokodowanie",
            ("pl", "OpenWeatherMap · current weather") => "OpenWeatherMap · bieżąca pogoda",
            ("pl", "Wikimedia Commons · image search and download") => {
                "Wikimedia Commons · wyszukiwanie i pobieranie obrazów"
            }
            ("da", "Japanese") => "Japansk",
            ("da", "Korean") => "Koreansk",
            ("da", "Reload now") => "Genindlæs nu",
            ("da", "TOTAL POWER INPUT") => "SAMLET EFFEKT IND",
            ("da", "TOTAL POWER OUTPUT") => "SAMLET EFFEKT UD",
            ("da", "TOTAL ENERGY") => "SAMLET ENERGI",
            ("da", "Open-Meteo · forecast") => "Open-Meteo · vejrudsigt",
            ("da", "Open-Meteo · geocoding") => "Open-Meteo · geokodning",
            ("da", "OpenWeatherMap · current weather") => "OpenWeatherMap · aktuelt vejr",
            ("da", "Wikimedia Commons · image search and download") => {
                "Wikimedia Commons · billedsøgning og download"
            }
            ("no", "Japanese") => "Japansk",
            ("no", "Korean") => "Koreansk",
            ("no", "Reload now") => "Last inn på nytt",
            ("no", "TOTAL POWER INPUT") => "TOTAL EFFEKT INN",
            ("no", "TOTAL POWER OUTPUT") => "TOTAL EFFEKT UT",
            ("no", "TOTAL ENERGY") => "TOTAL ENERGI",
            ("no", "Open-Meteo · forecast") => "Open-Meteo · værvarsel",
            ("no", "Open-Meteo · geocoding") => "Open-Meteo · geokoding",
            ("no", "OpenWeatherMap · current weather") => "OpenWeatherMap · nåværende vær",
            ("no", "Wikimedia Commons · image search and download") => {
                "Wikimedia Commons · bildesøk og nedlasting"
            }
            ("ja", "AVERAGE TEMPERATURE") => "平均気温",
            ("ko", "AVERAGE TEMPERATURE") => "평균 온도",
            ("ja", "Adjusts the power and temperature text in the panel") => {
                "パネル内の電力・温度テキストのサイズを調整します"
            }
            ("ko", "Adjusts the power and temperature text in the panel") => {
                "패널의 전력 및 온도 글자 크기를 조정합니다"
            }
            ("ja", "Base URL not configured") => "ベース URL が設定されていません",
            ("ko", "Base URL not configured") => "기본 URL이 설정되지 않았습니다",
            ("ja", "CONNECTION ERROR") => "接続エラー",
            ("ko", "CONNECTION ERROR") => "연결 오류",
            ("ja", "CURRENT POWER") => "現在の電力",
            ("ko", "CURRENT POWER") => "현재 전력",
            ("ja", "Combine consumption and solar power") => "消費電力と太陽光発電を合算",
            ("ko", "Combine consumption and solar power") => "소비 전력과 태양광 발전량 합산",
            ("ja", "Configured devices") => "設定済みデバイス",
            ("ko", "Configured devices") => "구성된 장치",
            ("ja", "Connected · device data loaded") => {
                "接続済み · デバイスデータを読み込みました"
            }
            ("ko", "Connected · device data loaded") => "연결됨 · 장치 데이터를 불러왔습니다",
            ("ja", "Controls the gap between panel items") => "パネル項目間の間隔を調整します",
            ("ko", "Controls the gap between panel items") => "패널 항목 사이 간격을 조정합니다",
            ("ja", "DOWNLOAD HISTORY") => "ダウンロード履歴",
            ("ko", "DOWNLOAD HISTORY") => "다운로드 기록",
            ("ja", "Device list and meter readings") => "デバイス一覧と計測値",
            ("ko", "Device list and meter readings") => "장치 목록 및 측정값",
            ("ja", "Display a separate refresh button beside the applet in the panel.") => {
                "パネルのアプレット横に更新ボタンを表示します。"
            }
            ("ko", "Display a separate refresh button beside the applet in the panel.") => {
                "패널 애플릿 옆에 별도의 새로 고침 버튼을 표시합니다."
            }
            (
                "ja",
                "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition.",
            ) => "写真はクロスフェードの前に5秒から5分間表示されます。",
            (
                "ko",
                "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition.",
            ) => "사진은 페이드 전환 전에 5초에서 5분 동안 표시됩니다.",
            ("ja", "Exclude solar devices") => "太陽光発電デバイスを除外",
            ("ko", "Exclude solar devices") => "태양광 장치 제외",
            ("ja", "External resources") => "外部サービス",
            ("ko", "External resources") => "외부 서비스",
            ("ja", "FORECAST UNAVAILABLE") => "予報を利用できません",
            ("ko", "FORECAST UNAVAILABLE") => "예보를 사용할 수 없습니다",
            ("ja", "FRITZ!Box devices unavailable") => "FRITZ!Box デバイスを利用できません",
            ("ko", "FRITZ!Box devices unavailable") => "FRITZ!Box 장치를 사용할 수 없습니다",
            ("ja", "Forecast loaded") => "予報を読み込みました",
            ("ko", "Forecast loaded") => "예보를 불러왔습니다",
            ("ja", "Forecast request in progress") => "予報を取得中です",
            ("ko", "Forecast request in progress") => "예보 요청 중",
            ("ja", "Fritz Power Monitor notification test") => "Fritz Power Monitor 通知テスト",
            ("ko", "Fritz Power Monitor notification test") => "Fritz Power Monitor 알림 테스트",
            ("ja", "GRAPH HISTORY") => "グラフ履歴",
            ("ko", "GRAPH HISTORY") => "그래프 기록",
            ("ja", "GRAPH STYLE") => "グラフ形式",
            ("ko", "GRAPH STYLE") => "그래프 스타일",
            ("ja", "INTERNET BANDWIDTH") => "インターネット帯域幅",
            ("ko", "INTERNET BANDWIDTH") => "인터넷 대역폭",
            (
                "ja",
                "Ignore temperatures from devices whose name contains “solar” when calculating the average.",
            ) => "名前に「solar」を含むデバイスの温度を平均値の計算から除外します。",
            (
                "ko",
                "Ignore temperatures from devices whose name contains “solar” when calculating the average.",
            ) => "이름에 “solar”가 포함된 장치의 온도는 평균 계산에서 제외합니다.",
            ("ja", "Image loaded") => "画像を読み込みました",
            ("ko", "Image loaded") => "이미지를 불러왔습니다",
            (
                "ja",
                "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.",
            ) => "FRITZ!Box の更新とは独立しています。既定は10分です。APIキーと場所が必要です。",
            (
                "ko",
                "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.",
            ) => {
                "FRITZ!Box 새로 고침과 별도로 작동하며 기본값은 10분입니다. API 키와 위치가 필요합니다."
            }
            ("ja", "Latest request failed") => "直近のリクエストに失敗しました",
            ("ko", "Latest request failed") => "최근 요청 실패",
            ("ja", "Latest request succeeded") => "直近のリクエストに成功しました",
            ("ko", "Latest request succeeded") => "최근 요청 성공",
            ("ja", "Location (city or latitude,longitude)") => "場所（都市名または緯度,経度）",
            ("ko", "Location (city or latitude,longitude)") => "위치(도시 또는 위도,경도)",
            ("ja", "Move down") => "下へ移動",
            ("ko", "Move down") => "아래로 이동",
            ("ja", "Move up") => "上へ移動",
            ("ko", "Move up") => "위로 이동",
            ("ja", "Multiline panel values") => "パネル値を複数行で表示",
            ("ko", "Multiline panel values") => "패널 값을 여러 줄로 표시",
            ("ja", "No usable image loaded") => "利用可能な画像を読み込めませんでした",
            ("ko", "No usable image loaded") => "사용 가능한 이미지를 불러오지 못했습니다",
            ("ja", "Not checked yet") => "未確認",
            ("ko", "Not checked yet") => "아직 확인하지 않음",
            ("ja", "Not configured") => "未設定",
            ("ko", "Not configured") => "구성되지 않음",
            ("ja", "Notifications are working correctly.") => "通知は正常に動作しています。",
            ("ko", "Notifications are working correctly.") => "알림이 정상적으로 작동합니다.",
            ("ja", "OpenWeatherMap API key") => "OpenWeatherMap APIキー",
            ("ko", "OpenWeatherMap API key") => "OpenWeatherMap API 키",
            ("ja", "Password") => "パスワード",
            ("ko", "Password") => "비밀번호",
            ("ja", "Per-device switch state") => "デバイスごとのスイッチ状態",
            ("ko", "Per-device switch state") => "장치별 스위치 상태",
            ("ja", "Plain HTTP · TLS is not enabled") => "HTTP接続 · TLSは無効です",
            ("ko", "Plain HTTP · TLS is not enabled") => "일반 HTTP · TLS가 활성화되지 않음",
            ("ja", "Reload now") => "今すぐ再読み込み",
            ("ko", "Reload now") => "지금 다시 불러오기",
            ("ja", "Request in progress") => "リクエスト処理中",
            ("ko", "Request in progress") => "요청 처리 중",
            ("ja", "Session authentication") => "セッション認証",
            ("ko", "Session authentication") => "세션 인증",
            ("ja", "Show panel refresh button") => "パネルの更新ボタンを表示",
            ("ko", "Show panel refresh button") => "패널 새로 고침 버튼 표시",
            ("ja", "Slideshow image duration") => "スライドショー画像の表示時間",
            ("ko", "Slideshow image duration") => "슬라이드쇼 이미지 표시 시간",
            ("ja", "Stack related readings vertically instead of placing them on one line.") => {
                "関連する測定値を横並びではなく縦に表示します。"
            }
            ("ko", "Stack related readings vertically instead of placing them on one line.") => {
                "관련 측정값을 한 줄에 나란히 표시하지 않고 세로로 쌓습니다."
            }
            ("ja", "System certificate roots") => "システムの証明書ストア",
            ("ko", "System certificate roots") => "시스템 인증서 저장소",
            ("ja", "TOTAL POWER INPUT") => "総入力電力",
            ("ko", "TOTAL POWER INPUT") => "총 입력 전력",
            ("ja", "TOTAL POWER OUTPUT") => "総出力電力",
            ("ja", "TOTAL ENERGY") => "総エネルギー",
            ("ko", "TOTAL POWER OUTPUT") => "총 출력 전력",
            ("ko", "TOTAL ENERGY") => "총 에너지",
            ("ja", "UPLOAD HISTORY") => "アップロード履歴",
            ("ko", "UPLOAD HISTORY") => "업로드 기록",
            ("ja", "Use current weather icon") => "現在の天気アイコンを使用",
            ("ko", "Use current weather icon") => "현재 날씨 아이콘 사용",
            ("ja", "Username") => "ユーザー名",
            ("ko", "Username") => "사용자 이름",
            ("ja", "Weather location not configured") => "天気の場所が設定されていません",
            ("ko", "Weather location not configured") => "날씨 위치가 설정되지 않았습니다",
            ("ja", "Weather refresh") => "天気の更新間隔",
            ("ko", "Weather refresh") => "날씨 새로 고침",
            ("ja", "Weather request failed") => "天気情報の取得に失敗しました",
            ("ko", "Weather request failed") => "날씨 요청 실패",
            ("es", "LAST") => "ÚLTIMOS",
            (
                "es",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean.",
            ) => {
                "El sistema sigue el idioma del escritorio. Idiomas disponibles: inglés, alemán, español, neerlandés, francés, italiano, sueco, portugués, polaco, danés, noruego, japonés y coreano."
            }
            ("es", "rain") => "lluvia",
            ("nl", "LAST") => "LAATSTE",
            (
                "nl",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean.",
            ) => {
                "Het systeem volgt de taal van je bureaublad. Beschikbare talen: Engels, Duits, Spaans, Nederlands, Frans, Italiaans, Zweeds, Portugees, Pools, Deens, Noors, Japans en Koreaans."
            }
            ("nl", "rain") => "regen",
            ("fr", "LAST") => "DERNIÈRES",
            (
                "fr",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean.",
            ) => {
                "Le système suit la langue de votre bureau. Langues disponibles : anglais, allemand, espagnol, néerlandais, français, italien, suédois, portugais, polonais, danois, norvégien, japonais et coréen."
            }
            ("fr", "rain") => "pluie",
            ("it", "LAST") => "ULTIMI",
            (
                "it",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean.",
            ) => {
                "Il sistema segue la lingua del desktop. Lingue disponibili: inglese, tedesco, spagnolo, olandese, francese, italiano, svedese, portoghese, polacco, danese, norvegese, giapponese e coreano."
            }
            ("it", "rain") => "pioggia",
            ("sv", "LAST") => "SENASTE",
            (
                "sv",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean.",
            ) => {
                "Systemet följer skrivbordets språk. Tillgängliga språk: engelska, tyska, spanska, nederländska, franska, italienska, svenska, portugisiska, polska, danska, norska, japanska och koreanska."
            }
            ("sv", "rain") => "regn",
            ("pt", "LAST") => "ÚLTIMOS",
            (
                "pt",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean.",
            ) => {
                "O sistema segue o idioma do ambiente de trabalho. Idiomas disponíveis: inglês, alemão, espanhol, neerlandês, francês, italiano, sueco, português, polaco, dinamarquês, norueguês, japonês e coreano."
            }
            ("pt", "rain") => "chuva",
            ("pl", "LAST") => "OSTATNIE",
            (
                "pl",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean.",
            ) => {
                "System używa języka pulpitu. Dostępne języki: angielski, niemiecki, hiszpański, niderlandzki, francuski, włoski, szwedzki, portugalski, polski, duński, norweski, japoński i koreański."
            }
            ("pl", "rain") => "deszcz",
            ("da", "LAST") => "SENESTE",
            (
                "da",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean.",
            ) => {
                "Systemet følger sproget på skrivebordet. Tilgængelige sprog: engelsk, tysk, spansk, nederlandsk, fransk, italiensk, svensk, portugisisk, polsk, dansk, norsk, japansk og koreansk."
            }
            ("da", "rain") => "regn",
            ("no", "LAST") => "SISTE",
            (
                "no",
                "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean.",
            ) => {
                "Systemet følger språket på skrivebordet. Tilgjengelige språk: engelsk, tysk, spansk, nederlandsk, fransk, italiensk, svensk, portugisisk, polsk, dansk, norsk, japansk og koreansk."
            }
            ("no", "rain") => "regn",
            ("ja", "%Y-%m-%d") => "%Y-%m-%d",
            ("ja", "FRITZ!Box") => "FRITZ!Box",
            ("ja", "HTTP") => "HTTP",
            ("ja", "LAST") => "直近",
            ("ja", "Open-Meteo · forecast") => "Open-Meteo · 予報",
            ("ja", "Open-Meteo · geocoding") => "Open-Meteo · ジオコーディング",
            ("ja", "OpenWeatherMap · current weather") => "OpenWeatherMap · 現在の天気",
            ("ja", "Photo") => "写真",
            ("ja", "Wikimedia Commons · image search and download") => {
                "Wikimedia Commons · 画像検索とダウンロード"
            }
            ("ja", "https://fritz.box") => "https://fritz.box",
            ("ja", "rain") => "雨",
            ("ko", "%Y-%m-%d") => "%Y-%m-%d",
            ("ko", "FRITZ!Box") => "FRITZ!Box",
            ("ko", "HTTP") => "HTTP",
            ("ko", "LAST") => "최근",
            ("ko", "Open-Meteo · forecast") => "Open-Meteo · 예보",
            ("ko", "Open-Meteo · geocoding") => "Open-Meteo · 지오코딩",
            ("ko", "OpenWeatherMap · current weather") => "OpenWeatherMap · 현재 날씨",
            ("ko", "Photo") => "사진",
            ("ko", "Wikimedia Commons · image search and download") => {
                "Wikimedia Commons · 이미지 검색 및 다운로드"
            }
            ("ko", "https://fritz.box") => "https://fritz.box",
            ("ko", "rain") => "비",
            ("ja", "HTTP failed") => "HTTPエラー",
            ("ko", "HTTP failed") => "HTTP 실패",
            ("ja", "HTTP successful") => "HTTP成功",
            ("ko", "HTTP successful") => "HTTP 성공",
            ("ja", "HTTPS / TLS") => "HTTPS / TLS",
            ("ko", "HTTPS / TLS") => "HTTPS / TLS",
            ("ja", "Load devices") => "デバイスを読み込む",
            ("ko", "Load devices") => "장치 불러오기",
            ("ja", "e.g. Amsterdam or 52.37,4.90") => "例：アムステルダム または 52.37,4.90",
            ("ko", "e.g. Amsterdam or 52.37,4.90") => "예: 암스테르담 또는 52.37,4.90",
            ("es", "Interface") => "Interfaz",
            ("nl", "Interface") => "Interface",
            ("fr", "Interface") => "Interface",
            ("it", "Interface") => "Interfaccia",
            ("sv", "Interface") => "Gränssnitt",
            ("pt", "Interface") => "Interface",
            ("pl", "Interface") => "Interfejs",
            ("da", "Interface") => "Grænseflade",
            ("no", "Interface") => "Grensesnitt",
            ("ja", "Interface") => "インターフェース",
            ("ko", "Interface") => "인터페이스",
            ("es", "Language and presentation") => "Idioma y presentación",
            ("nl", "Language and presentation") => "Taal en weergave",
            ("fr", "Language and presentation") => "Langue et présentation",
            ("it", "Language and presentation") => "Lingua e presentazione",
            ("sv", "Language and presentation") => "Språk och presentation",
            ("pt", "Language and presentation") => "Idioma e apresentação",
            ("pl", "Language and presentation") => "Język i wygląd",
            ("da", "Language and presentation") => "Sprog og visning",
            ("no", "Language and presentation") => "Språk og visning",
            ("ja", "Language and presentation") => "言語と表示",
            ("ko", "Language and presentation") => "언어 및 표시",
            ("es", "Display & updates") => "Pantalla y actualizaciones",
            ("nl", "Display & updates") => "Weergave en updates",
            ("fr", "Display & updates") => "Affichage et mises à jour",
            ("it", "Display & updates") => "Visualizzazione e aggiornamenti",
            ("sv", "Display & updates") => "Visning och uppdateringar",
            ("pt", "Display & updates") => "Ecrã e atualizações",
            ("pl", "Display & updates") => "Wyświetlanie i aktualizacje",
            ("da", "Display & updates") => "Visning og opdateringer",
            ("no", "Display & updates") => "Visning og oppdateringer",
            ("ja", "Display & updates") => "表示と更新",
            ("ko", "Display & updates") => "표시 및 업데이트",
            ("es", "Panel, graph history and refresh behavior") => {
                "Panel, historial de gráficas y actualizaciones"
            }
            ("nl", "Panel, graph history and refresh behavior") => {
                "Paneel, grafiekgeschiedenis en vernieuwingsgedrag"
            }
            ("fr", "Panel, graph history and refresh behavior") => {
                "Panneau, historique des graphiques et actualisation"
            }
            ("it", "Panel, graph history and refresh behavior") => {
                "Pannello, cronologia dei grafici e aggiornamenti"
            }
            ("sv", "Panel, graph history and refresh behavior") => {
                "Panel, diagramhistorik och uppdateringsbeteende"
            }
            ("pt", "Panel, graph history and refresh behavior") => {
                "Painel, histórico dos gráficos e comportamento da atualização"
            }
            ("pl", "Panel, graph history and refresh behavior") => {
                "Panel, historia wykresów i odświeżanie"
            }
            ("da", "Panel, graph history and refresh behavior") => {
                "Panel, grafhistorik og opdateringsadfærd"
            }
            ("no", "Panel, graph history and refresh behavior") => {
                "Panel, grafhistorikk og oppdateringsatferd"
            }
            ("ja", "Panel, graph history and refresh behavior") => "パネル、グラフ履歴、更新動作",
            ("ko", "Panel, graph history and refresh behavior") => {
                "패널, 그래프 기록 및 새로 고침 동작"
            }
            ("es", "Data sources") => "Fuentes de datos",
            ("nl", "Data sources") => "Gegevensbronnen",
            ("fr", "Data sources") => "Sources de données",
            ("it", "Data sources") => "Fonti dati",
            ("sv", "Data sources") => "Datakällor",
            ("pt", "Data sources") => "Fontes de dados",
            ("pl", "Data sources") => "Źródła danych",
            ("da", "Data sources") => "Datakilder",
            ("no", "Data sources") => "Datakilder",
            ("ja", "Data sources") => "データソース",
            ("ko", "Data sources") => "데이터 소스",
            ("es", "FRITZ!Box devices and local weather information") => {
                "Dispositivos FRITZ!Box e información meteorológica local"
            }
            ("nl", "FRITZ!Box devices and local weather information") => {
                "FRITZ!Box-apparaten en lokale weersinformatie"
            }
            ("fr", "FRITZ!Box devices and local weather information") => {
                "Appareils FRITZ!Box et météo locale"
            }
            ("it", "FRITZ!Box devices and local weather information") => {
                "Dispositivi FRITZ!Box e informazioni meteo locali"
            }
            ("sv", "FRITZ!Box devices and local weather information") => {
                "FRITZ!Box-enheter och lokal väderinformation"
            }
            ("pt", "FRITZ!Box devices and local weather information") => {
                "Dispositivos FRITZ!Box e meteorologia local"
            }
            ("pl", "FRITZ!Box devices and local weather information") => {
                "Urządzenia FRITZ!Box i lokalne informacje pogodowe"
            }
            ("da", "FRITZ!Box devices and local weather information") => {
                "FRITZ!Box-enheder og lokale vejroplysninger"
            }
            ("no", "FRITZ!Box devices and local weather information") => {
                "FRITZ!Box-enheter og lokal værinformasjon"
            }
            ("ja", "FRITZ!Box devices and local weather information") => {
                "FRITZ!Box デバイスと地域の天気情報"
            }
            ("ko", "FRITZ!Box devices and local weather information") => {
                "FRITZ!Box 장치 및 지역 날씨 정보"
            }
            ("es", "Layout") => "Diseño",
            ("nl", "Layout") => "Indeling",
            ("fr", "Layout") => "Disposition",
            ("it", "Layout") => "Disposizione",
            ("sv", "Layout") => "Layout",
            ("pt", "Layout") => "Disposição",
            ("pl", "Layout") => "Układ",
            ("da", "Layout") => "Layout",
            ("no", "Layout") => "Oppsett",
            ("ja", "Layout") => "レイアウト",
            ("ko", "Layout") => "레이아웃",
            ("es", "Choose what appears and in which order") => "Elige qué aparece y en qué orden",
            ("nl", "Choose what appears and in which order") => {
                "Kies wat wordt weergegeven en in welke volgorde"
            }
            ("fr", "Choose what appears and in which order") => {
                "Choisissez les éléments affichés et leur ordre"
            }
            ("it", "Choose what appears and in which order") => {
                "Scegli cosa mostrare e in quale ordine"
            }
            ("sv", "Choose what appears and in which order") => {
                "Välj vad som visas och i vilken ordning"
            }
            ("pt", "Choose what appears and in which order") => {
                "Escolha o que aparece e em que ordem"
            }
            ("pl", "Choose what appears and in which order") => {
                "Wybierz, co i w jakiej kolejności jest wyświetlane"
            }
            ("da", "Choose what appears and in which order") => {
                "Vælg, hvad der vises, og i hvilken rækkefølge"
            }
            ("no", "Choose what appears and in which order") => {
                "Velg hva som vises og i hvilken rekkefølge"
            }
            ("ja", "Choose what appears and in which order") => "表示する項目と順序を選択",
            ("ko", "Choose what appears and in which order") => "표시할 항목과 순서를 선택하세요",
            ("es", "Support") => "Asistencia",
            ("nl", "Support") => "Ondersteuning",
            ("fr", "Support") => "Assistance",
            ("it", "Support") => "Supporto",
            ("sv", "Support") => "Support",
            ("pt", "Support") => "Suporte",
            ("pl", "Support") => "Pomoc",
            ("da", "Support") => "Support",
            ("no", "Support") => "Støtte",
            ("ja", "Support") => "サポート",
            ("ko", "Support") => "지원",
            ("es", "Connection health, diagnostics and version information") => {
                "Estado de conexión, diagnóstico e información de versión"
            }
            ("nl", "Connection health, diagnostics and version information") => {
                "Verbindingsstatus, diagnose en versiegegevens"
            }
            ("fr", "Connection health, diagnostics and version information") => {
                "État de la connexion, diagnostics et informations de version"
            }
            ("it", "Connection health, diagnostics and version information") => {
                "Stato della connessione, diagnostica e informazioni sulla versione"
            }
            ("sv", "Connection health, diagnostics and version information") => {
                "Anslutningsstatus, diagnostik och versionsinformation"
            }
            ("pt", "Connection health, diagnostics and version information") => {
                "Estado da ligação, diagnóstico e informações da versão"
            }
            ("pl", "Connection health, diagnostics and version information") => {
                "Stan połączenia, diagnostyka i informacje o wersji"
            }
            ("da", "Connection health, diagnostics and version information") => {
                "Forbindelsesstatus, diagnosticering og versionsoplysninger"
            }
            ("no", "Connection health, diagnostics and version information") => {
                "Tilkoblingsstatus, diagnostikk og versjonsinformasjon"
            }
            ("ja", "Connection health, diagnostics and version information") => {
                "接続状態、診断、バージョン情報"
            }
            ("ko", "Connection health, diagnostics and version information") => {
                "연결 상태, 진단 및 버전 정보"
            }
            ("es", "Welcome") => "Bienvenido",
            ("nl", "Welcome") => "Welkom",
            ("fr", "Welcome") => "Bienvenue",
            ("it", "Welcome") => "Benvenuto",
            ("sv", "Welcome") => "Välkommen",
            ("pt", "Welcome") => "Bem-vindo",
            ("pl", "Welcome") => "Witamy",
            ("da", "Welcome") => "Velkommen",
            ("no", "Welcome") => "Velkommen",
            ("ja", "Welcome") => "ようこそ",
            ("ko", "Welcome") => "환영합니다",
            ("es", "Connect your FRITZ!Box") => "Conecta tu FRITZ!Box",
            ("nl", "Connect your FRITZ!Box") => "Verbind je FRITZ!Box",
            ("fr", "Connect your FRITZ!Box") => "Connecter votre FRITZ!Box",
            ("it", "Connect your FRITZ!Box") => "Collega il FRITZ!Box",
            ("sv", "Connect your FRITZ!Box") => "Anslut din FRITZ!Box",
            ("pt", "Connect your FRITZ!Box") => "Ligue a sua FRITZ!Box",
            ("pl", "Connect your FRITZ!Box") => "Połącz FRITZ!Box",
            ("da", "Connect your FRITZ!Box") => "Tilslut din FRITZ!Box",
            ("no", "Connect your FRITZ!Box") => "Koble til FRITZ!Box",
            ("ja", "Connect your FRITZ!Box") => "FRITZ!Box に接続",
            ("ko", "Connect your FRITZ!Box") => "FRITZ!Box 연결",
            ("es", "Choose your devices") => "Elige tus dispositivos",
            ("nl", "Choose your devices") => "Kies je apparaten",
            ("fr", "Choose your devices") => "Choisir vos appareils",
            ("it", "Choose your devices") => "Scegli i dispositivi",
            ("sv", "Choose your devices") => "Välj dina enheter",
            ("pt", "Choose your devices") => "Escolha os seus dispositivos",
            ("pl", "Choose your devices") => "Wybierz urządzenia",
            ("da", "Choose your devices") => "Vælg dine enheder",
            ("no", "Choose your devices") => "Velg enhetene dine",
            ("ja", "Choose your devices") => "デバイスを選択",
            ("ko", "Choose your devices") => "장치 선택",
            ("es", "Optional weather") => "Tiempo opcional",
            ("nl", "Optional weather") => "Optioneel weer",
            ("fr", "Optional weather") => "Météo facultative",
            ("it", "Optional weather") => "Meteo facoltativo",
            ("sv", "Optional weather") => "Valfritt väder",
            ("pt", "Optional weather") => "Meteorologia opcional",
            ("pl", "Optional weather") => "Opcjonalna pogoda",
            ("da", "Optional weather") => "Valgfrit vejr",
            ("no", "Optional weather") => "Valgfritt vær",
            ("ja", "Optional weather") => "天気（任意）",
            ("ko", "Optional weather") => "선택적 날씨 설정",
            ("es", "You're ready to go") => "Todo está listo",
            ("nl", "You're ready to go") => "Je bent klaar",
            ("fr", "You're ready to go") => "Tout est prêt",
            ("it", "You're ready to go") => "È tutto pronto",
            ("sv", "You're ready to go") => "Allt är klart",
            ("pt", "You're ready to go") => "Está tudo pronto",
            ("pl", "You're ready to go") => "Wszystko gotowe",
            ("da", "You're ready to go") => "Du er klar",
            ("no", "You're ready to go") => "Du er klar",
            ("ja", "You're ready to go") => "準備完了",
            ("ko", "You're ready to go") => "준비가 완료되었습니다",
            ("es", "Let's get your power monitor connected.") => {
                "Vamos a conectar tu monitor de energía."
            }
            ("nl", "Let's get your power monitor connected.") => {
                "Laten we je energiemonitor verbinden."
            }
            ("fr", "Let's get your power monitor connected.") => {
                "Connectons votre moniteur de puissance."
            }
            ("it", "Let's get your power monitor connected.") => {
                "Colleghiamo il tuo monitor energetico."
            }
            ("sv", "Let's get your power monitor connected.") => "Vi ansluter din effektmätare.",
            ("pt", "Let's get your power monitor connected.") => {
                "Vamos ligar o seu monitor de energia."
            }
            ("pl", "Let's get your power monitor connected.") => "Połączmy monitor energii.",
            ("da", "Let's get your power monitor connected.") => "Lad os tilslutte din strømmåler.",
            ("no", "Let's get your power monitor connected.") => "La oss koble til strømmåleren.",
            ("ja", "Let's get your power monitor connected.") => "電力モニターを接続しましょう。",
            ("ko", "Let's get your power monitor connected.") => "전력 모니터를 연결해 보겠습니다.",
            (
                "es",
                "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings.",
            ) => {
                "Esta breve configuración se conectará a tu FRITZ!Box, buscará dispositivos domésticos inteligentes compatibles y permitirá configurar el tiempo local. Podrás cambiarlo todo más adelante en Ajustes."
            }
            (
                "nl",
                "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings.",
            ) => {
                "Deze korte installatie maakt verbinding met je FRITZ!Box, zoekt compatibele smarthomeapparaten en kan optioneel lokaal weer instellen. Je kunt alles later wijzigen in Instellingen."
            }
            (
                "fr",
                "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings.",
            ) => {
                "Cette configuration rapide se connectera à votre FRITZ!Box, recherchera les appareils domotiques compatibles et pourra configurer la météo locale. Vous pourrez tout modifier plus tard dans les paramètres."
            }
            (
                "it",
                "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings.",
            ) => {
                "Questa breve configurazione si collegherà al FRITZ!Box, cercherà i dispositivi domotici compatibili e potrà configurare il meteo locale. Potrai modificare tutto in seguito nelle Impostazioni."
            }
            (
                "sv",
                "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings.",
            ) => {
                "Den här korta installationen ansluter till din FRITZ!Box, söker efter kompatibla smarthem-enheter och kan konfigurera lokalt väder. Du kan ändra allt senare i Inställningar."
            }
            (
                "pt",
                "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings.",
            ) => {
                "Esta configuração rápida liga-se à sua FRITZ!Box, procura dispositivos domésticos inteligentes compatíveis e permite configurar a meteorologia local. Pode alterar tudo mais tarde nas Definições."
            }
            (
                "pl",
                "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings.",
            ) => {
                "Ten krótki proces konfiguracji połączy się z FRITZ!Box, wyszuka zgodne urządzenia inteligentnego domu i opcjonalnie skonfiguruje lokalną pogodę. Wszystko możesz później zmienić w Ustawieniach."
            }
            (
                "da",
                "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings.",
            ) => {
                "Denne korte opsætning opretter forbindelse til din FRITZ!Box, finder kompatible smart home-enheder og kan valgfrit konfigurere lokalt vejr. Du kan ændre alt senere under Indstillinger."
            }
            (
                "no",
                "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings.",
            ) => {
                "Dette korte oppsettet kobler til FRITZ!Box, finner kompatible smarthjem-enheter og kan konfigurere lokalt vær. Du kan endre alt senere i Innstillinger."
            }
            (
                "ja",
                "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings.",
            ) => {
                "このセットアップでは FRITZ!Box に接続し、対応するスマートホームデバイスを検索して、必要に応じて地域の天気を設定します。設定は後から変更できます。"
            }
            (
                "ko",
                "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings.",
            ) => {
                "이 간단한 설정은 FRITZ!Box에 연결하고 호환되는 스마트 홈 장치를 검색하며, 선택적으로 지역 날씨를 설정합니다. 모든 항목은 나중에 설정에서 변경할 수 있습니다."
            }
            ("es", "FRITZ!Box address") => "Dirección de FRITZ!Box",
            ("nl", "FRITZ!Box address") => "FRITZ!Box-adres",
            ("fr", "FRITZ!Box address") => "Adresse de la FRITZ!Box",
            ("it", "FRITZ!Box address") => "Indirizzo FRITZ!Box",
            ("sv", "FRITZ!Box address") => "FRITZ!Box-adress",
            ("pt", "FRITZ!Box address") => "Endereço da FRITZ!Box",
            ("pl", "FRITZ!Box address") => "Adres FRITZ!Box",
            ("da", "FRITZ!Box address") => "FRITZ!Box-adresse",
            ("no", "FRITZ!Box address") => "FRITZ!Box-adresse",
            ("ja", "FRITZ!Box address") => "FRITZ!Box アドレス",
            ("ko", "FRITZ!Box address") => "FRITZ!Box 주소",
            ("es", "http://fritz.box or https://fritz.box") => {
                "http://fritz.box o https://fritz.box"
            }
            ("nl", "http://fritz.box or https://fritz.box") => {
                "http://fritz.box of https://fritz.box"
            }
            ("fr", "http://fritz.box or https://fritz.box") => {
                "http://fritz.box ou https://fritz.box"
            }
            ("it", "http://fritz.box or https://fritz.box") => {
                "http://fritz.box o https://fritz.box"
            }
            ("sv", "http://fritz.box or https://fritz.box") => {
                "http://fritz.box eller https://fritz.box"
            }
            ("pt", "http://fritz.box or https://fritz.box") => {
                "http://fritz.box ou https://fritz.box"
            }
            ("pl", "http://fritz.box or https://fritz.box") => {
                "http://fritz.box lub https://fritz.box"
            }
            ("da", "http://fritz.box or https://fritz.box") => {
                "http://fritz.box eller https://fritz.box"
            }
            ("no", "http://fritz.box or https://fritz.box") => {
                "http://fritz.box eller https://fritz.box"
            }
            ("ja", "http://fritz.box or https://fritz.box") => {
                "http://fritz.box または https://fritz.box"
            }
            ("ko", "http://fritz.box or https://fritz.box") => {
                "http://fritz.box 또는 https://fritz.box"
            }
            (
                "es",
                "Use the FRITZ!Box user credentials that have permission to access smart-home devices.",
            ) => {
                "Usa las credenciales de un usuario de FRITZ!Box con permiso para acceder a los dispositivos domésticos inteligentes."
            }
            (
                "nl",
                "Use the FRITZ!Box user credentials that have permission to access smart-home devices.",
            ) => {
                "Gebruik de inloggegevens van een FRITZ!Box-gebruiker die toegang heeft tot smarthomeapparaten."
            }
            (
                "fr",
                "Use the FRITZ!Box user credentials that have permission to access smart-home devices.",
            ) => {
                "Utilisez les identifiants d’un compte FRITZ!Box autorisé à accéder aux appareils domotiques."
            }
            (
                "it",
                "Use the FRITZ!Box user credentials that have permission to access smart-home devices.",
            ) => {
                "Usa le credenziali di un utente FRITZ!Box autorizzato ad accedere ai dispositivi domotici."
            }
            (
                "sv",
                "Use the FRITZ!Box user credentials that have permission to access smart-home devices.",
            ) => {
                "Använd inloggningsuppgifter för en FRITZ!Box-användare med behörighet att komma åt smarthem-enheter."
            }
            (
                "pt",
                "Use the FRITZ!Box user credentials that have permission to access smart-home devices.",
            ) => {
                "Utilize as credenciais de um utilizador FRITZ!Box com permissão para aceder aos dispositivos domésticos inteligentes."
            }
            (
                "pl",
                "Use the FRITZ!Box user credentials that have permission to access smart-home devices.",
            ) => {
                "Użyj danych logowania użytkownika FRITZ!Box, który ma uprawnienia dostępu do urządzeń inteligentnego domu."
            }
            (
                "da",
                "Use the FRITZ!Box user credentials that have permission to access smart-home devices.",
            ) => {
                "Brug loginoplysningerne til en FRITZ!Box-bruger med tilladelse til at få adgang til smart home-enheder."
            }
            (
                "no",
                "Use the FRITZ!Box user credentials that have permission to access smart-home devices.",
            ) => {
                "Bruk påloggingsinformasjonen til en FRITZ!Box-bruker som har tilgang til smarthjem-enheter."
            }
            (
                "ja",
                "Use the FRITZ!Box user credentials that have permission to access smart-home devices.",
            ) => {
                "スマートホームデバイスへのアクセス権を持つ FRITZ!Box ユーザーの認証情報を使用してください。"
            }
            (
                "ko",
                "Use the FRITZ!Box user credentials that have permission to access smart-home devices.",
            ) => "스마트 홈 장치에 접근할 권한이 있는 FRITZ!Box 사용자 계정을 사용하세요.",
            (
                "es",
                "Discover compatible FRITZ! smart-home devices, then select the devices to monitor.",
            ) => {
                "Busca dispositivos domésticos inteligentes FRITZ! compatibles y selecciona los que quieras supervisar."
            }
            (
                "nl",
                "Discover compatible FRITZ! smart-home devices, then select the devices to monitor.",
            ) => {
                "Zoek naar compatibele FRITZ!-smarthomeapparaten en selecteer welke apparaten je wilt bewaken."
            }
            (
                "fr",
                "Discover compatible FRITZ! smart-home devices, then select the devices to monitor.",
            ) => {
                "Recherchez les appareils domotiques FRITZ! compatibles, puis sélectionnez ceux à surveiller."
            }
            (
                "it",
                "Discover compatible FRITZ! smart-home devices, then select the devices to monitor.",
            ) => {
                "Cerca i dispositivi domotici FRITZ! compatibili, quindi seleziona quelli da monitorare."
            }
            (
                "sv",
                "Discover compatible FRITZ! smart-home devices, then select the devices to monitor.",
            ) => {
                "Sök efter kompatibla FRITZ!-smarthem-enheter och välj sedan vilka enheter som ska övervakas."
            }
            (
                "pt",
                "Discover compatible FRITZ! smart-home devices, then select the devices to monitor.",
            ) => {
                "Procure dispositivos domésticos inteligentes FRITZ! compatíveis e selecione os que pretende monitorizar."
            }
            (
                "pl",
                "Discover compatible FRITZ! smart-home devices, then select the devices to monitor.",
            ) => {
                "Wyszukaj zgodne urządzenia inteligentnego domu FRITZ!, a następnie wybierz urządzenia do monitorowania."
            }
            (
                "da",
                "Discover compatible FRITZ! smart-home devices, then select the devices to monitor.",
            ) => {
                "Find kompatible FRITZ!-smart home-enheder, og vælg derefter de enheder, der skal overvåges."
            }
            (
                "no",
                "Discover compatible FRITZ! smart-home devices, then select the devices to monitor.",
            ) => {
                "Søk etter kompatible FRITZ!-smarthjem-enheter, og velg deretter enhetene som skal overvåkes."
            }
            (
                "ja",
                "Discover compatible FRITZ! smart-home devices, then select the devices to monitor.",
            ) => "対応する FRITZ! スマートホームデバイスを検索し、監視するデバイスを選択します。",
            (
                "ko",
                "Discover compatible FRITZ! smart-home devices, then select the devices to monitor.",
            ) => "호환되는 FRITZ! 스마트 홈 장치를 검색한 다음 모니터링할 장치를 선택하세요.",
            ("es", "Discover devices") => "Buscar dispositivos",
            ("nl", "Discover devices") => "Apparaten zoeken",
            ("fr", "Discover devices") => "Rechercher des appareils",
            ("it", "Discover devices") => "Cerca dispositivi",
            ("sv", "Discover devices") => "Sök efter enheter",
            ("pt", "Discover devices") => "Procurar dispositivos",
            ("pl", "Discover devices") => "Wyszukaj urządzenia",
            ("da", "Discover devices") => "Find enheder",
            ("no", "Discover devices") => "Søk etter enheter",
            ("ja", "Discover devices") => "デバイスを検索",
            ("ko", "Discover devices") => "장치 검색",
            ("es", "Searching your FRITZ!Box…") => "Buscando en tu FRITZ!Box…",
            ("nl", "Searching your FRITZ!Box…") => "Je FRITZ!Box doorzoeken…",
            ("fr", "Searching your FRITZ!Box…") => "Recherche sur votre FRITZ!Box…",
            ("it", "Searching your FRITZ!Box…") => "Ricerca nel FRITZ!Box…",
            ("sv", "Searching your FRITZ!Box…") => "Söker i din FRITZ!Box…",
            ("pt", "Searching your FRITZ!Box…") => "A procurar na sua FRITZ!Box…",
            ("pl", "Searching your FRITZ!Box…") => "Wyszukiwanie w FRITZ!Box…",
            ("da", "Searching your FRITZ!Box…") => "Søger på din FRITZ!Box…",
            ("no", "Searching your FRITZ!Box…") => "Søker i FRITZ!Box…",
            ("ja", "Searching your FRITZ!Box…") => "FRITZ!Box を検索中…",
            ("ko", "Searching your FRITZ!Box…") => "FRITZ!Box 검색 중…",
            (
                "es",
                "No compatible devices were found. You can continue and retry later in Settings → Devices.",
            ) => {
                "No se encontraron dispositivos compatibles. Puedes continuar e intentarlo de nuevo más tarde en Ajustes → Dispositivos."
            }
            (
                "nl",
                "No compatible devices were found. You can continue and retry later in Settings → Devices.",
            ) => {
                "Geen compatibele apparaten gevonden. Je kunt doorgaan en het later opnieuw proberen via Instellingen → Apparaten."
            }
            (
                "fr",
                "No compatible devices were found. You can continue and retry later in Settings → Devices.",
            ) => {
                "Aucun appareil compatible trouvé. Vous pouvez continuer et réessayer plus tard dans Paramètres → Appareils."
            }
            (
                "it",
                "No compatible devices were found. You can continue and retry later in Settings → Devices.",
            ) => {
                "Nessun dispositivo compatibile trovato. Puoi continuare e riprovare più tardi in Impostazioni → Dispositivi."
            }
            (
                "sv",
                "No compatible devices were found. You can continue and retry later in Settings → Devices.",
            ) => {
                "Inga kompatibla enheter hittades. Du kan fortsätta och försöka igen senare under Inställningar → Enheter."
            }
            (
                "pt",
                "No compatible devices were found. You can continue and retry later in Settings → Devices.",
            ) => {
                "Não foram encontrados dispositivos compatíveis. Pode continuar e tentar novamente mais tarde em Definições → Dispositivos."
            }
            (
                "pl",
                "No compatible devices were found. You can continue and retry later in Settings → Devices.",
            ) => {
                "Nie znaleziono zgodnych urządzeń. Możesz kontynuować i ponowić próbę później w Ustawienia → Urządzenia."
            }
            (
                "da",
                "No compatible devices were found. You can continue and retry later in Settings → Devices.",
            ) => {
                "Der blev ikke fundet kompatible enheder. Du kan fortsætte og prøve igen senere under Indstillinger → Enheder."
            }
            (
                "no",
                "No compatible devices were found. You can continue and retry later in Settings → Devices.",
            ) => {
                "Fant ingen kompatible enheter. Du kan fortsette og prøve igjen senere under Innstillinger → Enheter."
            }
            (
                "ja",
                "No compatible devices were found. You can continue and retry later in Settings → Devices.",
            ) => {
                "対応デバイスが見つかりません。続行して、後で「設定」→「デバイス」から再試行できます。"
            }
            (
                "ko",
                "No compatible devices were found. You can continue and retry later in Settings → Devices.",
            ) => {
                "호환되는 장치를 찾지 못했습니다. 계속 진행한 다음 설정 → 장치에서 다시 시도할 수 있습니다."
            }
            ("es", "Enable local weather (optional)") => "Activar el tiempo local (opcional)",
            ("nl", "Enable local weather (optional)") => "Lokaal weer inschakelen (optioneel)",
            ("fr", "Enable local weather (optional)") => "Activer la météo locale (facultatif)",
            ("it", "Enable local weather (optional)") => "Attiva il meteo locale (facoltativo)",
            ("sv", "Enable local weather (optional)") => "Aktivera lokalt väder (valfritt)",
            ("pt", "Enable local weather (optional)") => "Ativar meteorologia local (opcional)",
            ("pl", "Enable local weather (optional)") => "Włącz lokalną pogodę (opcjonalnie)",
            ("da", "Enable local weather (optional)") => "Aktivér lokalt vejr (valgfrit)",
            ("no", "Enable local weather (optional)") => "Aktiver lokalt vær (valgfritt)",
            ("ja", "Enable local weather (optional)") => "地域の天気を有効にする（任意）",
            ("ko", "Enable local weather (optional)") => "지역 날씨 사용(선택 사항)",
            ("es", "Show weather") => "Mostrar el tiempo",
            ("nl", "Show weather") => "Weer tonen",
            ("fr", "Show weather") => "Afficher la météo",
            ("it", "Show weather") => "Mostra meteo",
            ("sv", "Show weather") => "Visa väder",
            ("pt", "Show weather") => "Mostrar meteorologia",
            ("pl", "Show weather") => "Pokaż pogodę",
            ("da", "Show weather") => "Vis vejr",
            ("no", "Show weather") => "Vis vær",
            ("ja", "Show weather") => "天気を表示",
            ("ko", "Show weather") => "날씨 표시",
            (
                "es",
                "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later.",
            ) => {
                "El tiempo funciona independientemente de la supervisión de FRITZ!Box. Puedes omitir este paso y configurarlo más tarde."
            }
            (
                "nl",
                "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later.",
            ) => {
                "Weer werkt onafhankelijk van de FRITZ!Box-monitoring. Je kunt deze stap overslaan en later instellen."
            }
            (
                "fr",
                "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later.",
            ) => {
                "La météo est indépendante de la surveillance FRITZ!Box. Vous pouvez ignorer cette étape et la configurer plus tard."
            }
            (
                "it",
                "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later.",
            ) => {
                "Il meteo è indipendente dal monitoraggio FRITZ!Box. Puoi saltare questo passaggio e configurarlo in seguito."
            }
            (
                "sv",
                "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later.",
            ) => {
                "Vädret är oberoende av FRITZ!Box-övervakningen. Du kan hoppa över detta och konfigurera det senare."
            }
            (
                "pt",
                "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later.",
            ) => {
                "A meteorologia é independente da monitorização da FRITZ!Box. Pode ignorar este passo e configurá-la mais tarde."
            }
            (
                "pl",
                "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later.",
            ) => {
                "Pogoda działa niezależnie od monitorowania FRITZ!Box. Możesz pominąć ten krok i skonfigurować ją później."
            }
            (
                "da",
                "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later.",
            ) => {
                "Vejret er uafhængigt af FRITZ!Box-overvågningen. Du kan springe dette trin over og konfigurere det senere."
            }
            (
                "no",
                "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later.",
            ) => {
                "Været er uavhengig av FRITZ!Box-overvåkingen. Du kan hoppe over dette og konfigurere det senere."
            }
            (
                "ja",
                "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later.",
            ) => {
                "天気情報は FRITZ!Box の監視とは独立しています。この手順を省略して後で設定できます。"
            }
            (
                "ko",
                "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later.",
            ) => {
                "날씨 정보는 FRITZ!Box 모니터링과 별개입니다. 이 단계를 건너뛰고 나중에 설정할 수 있습니다."
            }
            ("es", "Setup is complete. Your choices are saved automatically.") => {
                "La configuración ha terminado. Tus opciones se guardan automáticamente."
            }
            ("nl", "Setup is complete. Your choices are saved automatically.") => {
                "De installatie is voltooid. Je keuzes worden automatisch opgeslagen."
            }
            ("fr", "Setup is complete. Your choices are saved automatically.") => {
                "La configuration est terminée. Vos choix sont enregistrés automatiquement."
            }
            ("it", "Setup is complete. Your choices are saved automatically.") => {
                "Configurazione completata. Le tue scelte vengono salvate automaticamente."
            }
            ("sv", "Setup is complete. Your choices are saved automatically.") => {
                "Installationen är klar. Dina val sparas automatiskt."
            }
            ("pt", "Setup is complete. Your choices are saved automatically.") => {
                "A configuração está concluída. As suas escolhas são guardadas automaticamente."
            }
            ("pl", "Setup is complete. Your choices are saved automatically.") => {
                "Konfiguracja zakończona. Twoje wybory są zapisywane automatycznie."
            }
            ("da", "Setup is complete. Your choices are saved automatically.") => {
                "Opsætningen er færdig. Dine valg gemmes automatisk."
            }
            ("no", "Setup is complete. Your choices are saved automatically.") => {
                "Oppsettet er fullført. Valgene dine lagres automatisk."
            }
            ("ja", "Setup is complete. Your choices are saved automatically.") => {
                "セットアップが完了しました。選択内容は自動的に保存されます。"
            }
            ("ko", "Setup is complete. Your choices are saved automatically.") => {
                "설정이 완료되었습니다. 선택 사항은 자동으로 저장됩니다."
            }
            ("es", "devices selected") => "dispositivos seleccionados",
            ("nl", "devices selected") => "apparaten geselecteerd",
            ("fr", "devices selected") => "appareils sélectionnés",
            ("it", "devices selected") => "dispositivi selezionati",
            ("sv", "devices selected") => "enheter valda",
            ("pt", "devices selected") => "dispositivos selecionados",
            ("pl", "devices selected") => "wybrane urządzenia",
            ("da", "devices selected") => "enheder valgt",
            ("no", "devices selected") => "enheter valgt",
            ("ja", "devices selected") => "台のデバイスを選択中",
            ("ko", "devices selected") => "개 장치 선택됨",
            ("es", "Back") => "Atrás",
            ("nl", "Back") => "Terug",
            ("fr", "Back") => "Retour",
            ("it", "Back") => "Indietro",
            ("sv", "Back") => "Tillbaka",
            ("pt", "Back") => "Voltar",
            ("pl", "Back") => "Wstecz",
            ("da", "Back") => "Tilbage",
            ("no", "Back") => "Tilbake",
            ("ja", "Back") => "戻る",
            ("ko", "Back") => "뒤로",
            ("es", "Finish") => "Finalizar",
            ("nl", "Finish") => "Voltooien",
            ("fr", "Finish") => "Terminer",
            ("it", "Finish") => "Fine",
            ("sv", "Finish") => "Slutför",
            ("pt", "Finish") => "Concluir",
            ("pl", "Finish") => "Zakończ",
            ("da", "Finish") => "Afslut",
            ("no", "Finish") => "Fullfør",
            ("ja", "Finish") => "完了",
            ("ko", "Finish") => "완료",
            ("es", "Finish setup") => "Terminar configuración",
            ("nl", "Finish setup") => "Installatie afronden",
            ("fr", "Finish setup") => "Terminer la configuration",
            ("it", "Finish setup") => "Completa configurazione",
            ("sv", "Finish setup") => "Slutför installationen",
            ("pt", "Finish setup") => "Concluir configuração",
            ("pl", "Finish setup") => "Zakończ konfigurację",
            ("da", "Finish setup") => "Afslut opsætning",
            ("no", "Finish setup") => "Fullfør oppsettet",
            ("ja", "Finish setup") => "セットアップを完了",
            ("ko", "Finish setup") => "설정 마치기",
            ("es", "Continue") => "Continuar",
            ("nl", "Continue") => "Doorgaan",
            ("fr", "Continue") => "Continuer",
            ("it", "Continue") => "Continua",
            ("sv", "Continue") => "Fortsätt",
            ("pt", "Continue") => "Continuar",
            ("pl", "Continue") => "Dalej",
            ("da", "Continue") => "Fortsæt",
            ("no", "Continue") => "Fortsett",
            ("ja", "Continue") => "続行",
            ("ko", "Continue") => "계속",
            ("es", "FRITZ!Box connection") => "Conexión de FRITZ!Box",
            ("nl", "FRITZ!Box connection") => "FRITZ!Box-verbinding",
            ("fr", "FRITZ!Box connection") => "Connexion FRITZ!Box",
            ("it", "FRITZ!Box connection") => "Connessione FRITZ!Box",
            ("sv", "FRITZ!Box connection") => "FRITZ!Box-anslutning",
            ("pt", "FRITZ!Box connection") => "Ligação FRITZ!Box",
            ("pl", "FRITZ!Box connection") => "Połączenie FRITZ!Box",
            ("da", "FRITZ!Box connection") => "FRITZ!Box-forbindelse",
            ("no", "FRITZ!Box connection") => "FRITZ!Box-tilkobling",
            ("ja", "FRITZ!Box connection") => "FRITZ!Box 接続",
            ("ko", "FRITZ!Box connection") => "FRITZ!Box 연결",
            _ => key,
        };
    }
    match key {
        "Settings" => "Einstellungen",
        "Configure your power monitor" => "Strommonitor konfigurieren",
        "GENERAL" => "ALLGEMEIN",
        "Language" => "Sprache",
        "Choose the interface language" => "Sprache der Benutzeroberfläche auswählen",
        "Panel" => "Panel",
        "Graph history" => "Diagrammverlauf",
        "Automatic refresh" => "Automatische Aktualisierung",
        "Weather" => "Wetter",
        "Photo" => "Foto:",
        "Connection" => "Verbindung",
        "Devices" => "Geräte",
        "Menu order" => "Menüreihenfolge",
        "Panel order" => "Panel-Reihenfolge",
        "Diagnostics" => "Diagnose",
        "Appearance and live readings in the panel" => "Darstellung und Live-Messwerte im Panel",
        "Choose how much power history is displayed" => "Zeitraum des Leistungsverlaufs festlegen",
        "Control how often device readings update" => {
            "Aktualisierungsintervall der Messwerte festlegen"
        }
        "Show local weather alongside power readings" => {
            "Lokales Wetter neben den Leistungsmesswerten anzeigen"
        }
        "FRITZ!Box address and security settings" => {
            "FRITZ!Box-Adresse und Sicherheitseinstellungen"
        }
        "Select and configure devices" => "Geräte auswählen und konfigurieren",
        "Choose the order of popup elements" => "Reihenfolge der Popup-Elemente festlegen",
        "Arrange individual panel readings" => "Einzelne Panel-Messwerte anordnen",
        "Connection and settings status" => "Verbindungs- und Einstellungsstatus",
        "System default" => "Systemstandard",
        "English" => "Englisch",
        "German" => "Deutsch",
        "Spanish" => "Spanisch",
        "Dutch" => "Niederländisch",
        "French" => "Französisch",
        "Italian" => "Italienisch",
        "Swedish" => "Schwedisch",
        "Portuguese" => "Portugiesisch",
        "Polish" => "Polnisch",
        "Danish" => "Dänisch",
        "Norwegian" => "Norwegisch",
        "Deutsch" => "Deutsch",
        "Interface language" => "Sprache der Benutzeroberfläche",
        "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish and Norwegian." => {
            "System folgt der Sprache Ihres Desktops. Verfügbare Sprachen: Englisch, Deutsch, Spanisch, Niederländisch, Französisch, Italienisch, Schwedisch, Portugiesisch, Polnisch, Dänisch und Norwegisch."
        }
        "Adjusts the power and temperature text in the panel" => {
            "Passt die Schriftgröße der Leistungs- und Temperaturwerte im Panel an"
        }
        "Controls the gap between panel items" => "Legt den Abstand zwischen Panel-Elementen fest",
        "Multiline panel values" => "Mehrzeilige Panel-Werte",
        "Stack related readings vertically instead of placing them on one line." => {
            "Zugehörige Messwerte untereinander statt in einer Zeile anzeigen."
        }
        "Show panel refresh button" => "Aktualisierungsschaltfläche im Panel anzeigen",
        "Display a separate refresh button beside the applet in the panel." => {
            "Eine separate Aktualisierungsschaltfläche neben dem Applet anzeigen."
        }
        "Show internet bandwidth" => "Internet-Bandbreite anzeigen",
        "Combine consumption and solar power" => "Stromverbrauch und Solarleistung kombinieren",
        "Show one net power value: total consumption minus solar generation." => {
            "Einen Nettoleistungswert anzeigen: Gesamtverbrauch minus Solarerzeugung."
        }
        "Average temperature" => "Durchschnittstemperatur",
        "Exclude solar devices" => "Solargeräte ausschließen",
        "Ignore temperatures from devices whose name contains “solar” when calculating the average." => {
            "Temperaturen von Geräten mit „solar“ im Namen bei der Durchschnittsberechnung ignorieren."
        }
        "GRAPH STYLE" => "DIAGRAMMSTIL",
        "30 seconds to 1 hour" => "30 Sekunden bis 1 Stunde",
        "Use current weather icon" => "Aktuelles Wettersymbol verwenden",
        "OpenWeatherMap API key" => "OpenWeatherMap-API-Schlüssel",
        "Location (city or latitude,longitude)" => "Standort (Stadt oder Breitengrad,Längengrad)",
        "Slideshow image duration" => "Anzeigedauer je Diashow-Bild",
        "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition." => {
            "Jedes Foto wird vor dem Überblenden 5 Sekunden bis 5 Minuten angezeigt."
        }
        "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location." => {
            "Unabhängig von der FRITZ!Box-Aktualisierung; Standard: 10 Minuten. API-Schlüssel und Standort erforderlich."
        }
        "FRITZ!Box URL" => "FRITZ!Box-URL",
        "Username" => "Benutzername",
        "Password" => "Passwort",
        "Allow invalid TLS certificates" => "Ungültige TLS-Zertifikate zulassen",
        "Popup element order" => "Reihenfolge der Popup-Elemente",
        "Panel item order" => "Reihenfolge der Panel-Elemente",
        "Select power-meter devices" => "Strommessgeräte auswählen",
        "No devices match your search" => "Keine Geräte entsprechen der Suche",
        "Input device (separate total)" => "Einspeisegerät (separate Summe)",
        "Temperature alerts" => "Temperaturwarnungen",
        "No status messages yet" => "Noch keine Statusmeldungen",
        "Settings saved" => "Einstellungen gespeichert",
        "Wind speed" => "Windgeschwindigkeit",
        "Direction" => "Richtung",
        "Pressure" => "Luftdruck",
        "Forecast data by Open-Meteo" => "Vorhersagedaten von Open-Meteo",
        "Loading forecast…”" => "Vorhersage wird geladen…",
        "Weather data is loading…" => "Wetterdaten werden geladen…",
        "DOWNLOAD" => "DOWNLOAD",
        "UPLOAD" => "UPLOAD",
        "DOWNLOAD HISTORY" => "DOWNLOAD-VERLAUF",
        "UPLOAD HISTORY" => "UPLOAD-VERLAUF",
        "POWER (W)" => "LEISTUNG (W)",
        "Now" => "Jetzt",
        "TEMPERATURE (°C)" => "TEMPERATUR (°C)",
        "Select one or more devices in Settings" => {
            "Wählen Sie ein oder mehrere Geräte in den Einstellungen aus"
        }
        "SETTINGS" => "EINSTELLUNGEN",
        "QUICK SETTINGS" => "SCHNELLEINSTELLUNGEN",
        "Solar power" => "Solarleistung",
        "Internet bandwidth" => "Internet-Bandbreite",
        "Temperature" => "Temperatur",
        "Live power summary" => "Aktuelle Leistungsübersicht",
        "Total power out" => "Gesamte Leistungsabgabe",
        "Total power in" => "Gesamte Leistungseinspeisung",
        "Unknown item" => "Unbekanntes Element",
        "Unknown element" => "Unbekanntes Element",
        "Power consumption" => "Stromverbrauch",
        "Move up" => "Nach oben",
        "Move down" => "Nach unten",
        "Load devices" => "Geräte laden",
        "Search devices" => "Geräte suchen",
        "Refresh" => "Aktualisieren",
        "Visibility" => "Sichtweite",
        "Graph" => "Diagramm",
        "Connection error" => "Verbindungsfehler",
        "On" => "Ein",
        "Off" => "Aus",
        "Enabled" => "Aktiviert",
        "Disabled" => "Deaktiviert",
        "Loading forecast…" => "Vorhersage wird geladen…",
        "API key" => "API-Schlüssel",
        "https://fritz.box" => "https://fritz.box",
        "e.g. Amsterdam or 52.37,4.90" => "z. B. Berlin oder 52.52,13.40",
        "7d" => "7 T",
        "14d" => "14 T",
        "Refresh now" => "Jetzt aktualisieren",
        "Display the maximum download and upload link speeds reported by your FRITZ!Box connection in Mb/s." => {
            "Maximale Download- und Upload-Verbindungsgeschwindigkeit Ihrer FRITZ!Box in Mb/s anzeigen."
        }
        "5 minutes to 4 years. Saved samples and the selected retention period survive applet restarts. Sampling adapts to retention, capped at about 50,000 samples per series." => {
            "5 Minuten bis 4 Jahre. Gespeicherte Messwerte und der gewählte Zeitraum bleiben nach Neustarts erhalten. Die Abtastrate passt sich an, maximal etwa 50.000 Messwerte pro Reihe."
        }
        "✓ Line" => "✓ Linie",
        "Line" => "Linie",
        "✓ Bar" => "✓ Balken",
        "Bar" => "Balken",
        "Trusted CA certificate (PEM file, optional)" => {
            "Vertrauenswürdiges CA-Zertifikat (optionale PEM-Datei)"
        }
        "Leave empty to use the system certificate store" => {
            "Leer lassen, um den System-Zertifikatsspeicher zu verwenden"
        }
        "Move sections up or down. Changes are saved automatically." => {
            "Abschnitte nach oben oder unten verschieben. Änderungen werden automatisch gespeichert."
        }
        "Move each reading independently. Changes are saved automatically." => {
            "Jeden Messwert einzeln verschieben. Änderungen werden automatisch gespeichert."
        }
        "Search devices by name or AIN" => "Geräte nach Name oder AIN suchen",
        "Notify above the upper limit or below the lower limit" => {
            "Bei Überschreitung des oberen oder Unterschreitung des unteren Grenzwerts benachrichtigen"
        }
        "Application" => "Anwendung",
        "Version" => "Version",
        "Latest version" => "Neueste Version",
        "Check for updates" => "Auf Aktualisierungen prüfen",
        "Checking for updates…" => "Suche nach Aktualisierungen…",
        "Update available" => "Aktualisierung verfügbar",
        "Install update" => "Aktualisierung installieren",
        "Installing update…" => "Aktualisierung wird installiert…",
        "Update installation failed" => "Installation der Aktualisierung fehlgeschlagen",
        "Up to date" => "Aktuell",
        "Version check failed" => "Versionsprüfung fehlgeschlagen",
        "Connection security" => "Verbindungssicherheit",
        "HTTPS / TLS" => "HTTPS / TLS",
        "External resources" => "Externe Dienste",
        "Status from the latest real request. Opening Diagnostics does not trigger network requests." => {
            "Status der letzten tatsächlichen Anfrage. Das Öffnen der Diagnose löst keine Netzwerkanfragen aus."
        }
        "Latest request succeeded" => "Letzte Anfrage erfolgreich",
        "Latest request failed" => "Letzte Anfrage fehlgeschlagen",
        "Not checked yet" => "Noch nicht geprüft",
        "Not configured" => "Nicht konfiguriert",
        "OpenWeatherMap · current weather" => "OpenWeatherMap · aktuelles Wetter",
        "Wikimedia Commons · weather image" => "Wikimedia Commons · Wetterbild",
        "Open-Meteo · forecast and geocoding" => "Open-Meteo · Vorhersage und Geokodierung",
        "FRITZ!Box API" => "FRITZ!Box-API",
        "Session authentication" => "Sitzungsauthentifizierung",
        "Device list and meter readings" => "Geräteliste und Messwerte",
        "Per-device switch state" => "Schaltzustand je Gerät",
        "Configured devices: {}" => "Konfigurierte Geräte: {}",
        "Request in progress" => "Anfrage läuft",
        "Reload now" => "Jetzt neu laden",
        "WEATHER UNAVAILABLE" => "WETTER NICHT VERFÜGBAR",
        "SETTINGS NOT SAVED" => "EINSTELLUNGEN NICHT GESPEICHERT",
        "CONNECTION FAILED" => "VERBINDUNG FEHLGESCHLAGEN",
        "Feels like {:.1} °C" => "Gefühlt {:.1} °C",
        "Humidity  {}%" => "Luftfeuchtigkeit  {}%",
        "Clouds  {}%" => "Bewölkung  {}%",
        "Visibility  {:.1} km" => "Sichtweite  {:.1} km",
        "Sunrise  {sunrise}" => "Sonnenaufgang  {sunrise}",
        "Sunset  {sunset}" => "Sonnenuntergang  {sunset}",
        "{}-day forecast" => "{}-Tage-Vorhersage",
        "{}% rain" => "{}% Regen",
        "Waiting for bandwidth data…" => "Warte auf Bandbreitendaten…",
        "Bandwidth data unavailable" => "Bandbreitendaten nicht verfügbar",
        "LAST {}" => "LETZTE {}",
        "POWER" => "LEISTUNG",
        "CURRENT POWER" => "AKTUELLE LEISTUNG",
        "INTERNET BANDWIDTH" => "INTERNET-BANDBREITE",
        "CONNECTION ERROR" => "VERBINDUNGSFEHLER",
        "Refresh · {}" => "Aktualisierung · {}",
        "Graph · {}" => "Diagramm · {}",
        "FORECAST UNAVAILABLE" => "VORHERSAGE NICHT VERFÜGBAR",
        "GRAPH HISTORY" => "DIAGRAMMVERLAUF",
        "AUTOMATIC REFRESH" => "AUTOMATISCHE AKTUALISIERUNG",
        "Upper limit" => "Oberer Grenzwert",
        "Lower limit" => "Unterer Grenzwert",
        "Weather refresh" => "Wetteraktualisierung",
        "Feels like" => "Gefühlt",
        "Humidity" => "Luftfeuchtigkeit",
        "Clouds" => "Bewölkung",
        "Sunrise" => "Sonnenaufgang",
        "Sunset" => "Sonnenuntergang",
        "Certificate validation disabled" => "Zertifikatsprüfung deaktiviert",
        "Custom CA certificate configured" => "Benutzerdefiniertes CA-Zertifikat konfiguriert",
        "System certificate roots" => "System-Zertifikate",
        "Plain HTTP · TLS is not enabled" => "Einfaches HTTP · TLS ist nicht aktiviert",
        "URL is invalid or not configured" => "URL ungültig oder nicht konfiguriert",
        "Image loaded" => "Bild geladen",
        "No usable image loaded" => "Kein verwendbares Bild geladen",
        "Weather request failed" => "Wetteranfrage fehlgeschlagen",
        "Forecast loaded" => "Vorhersage geladen",
        "Not checked · request a forecast to test" => {
            "Nicht geprüft · Vorhersage zum Testen abrufen"
        }
        "HTTP" => "HTTP",
        "HTTP successful" => "erfolgreich",
        "HTTP failed" => "fehlgeschlagen",
        "Connected · device data loaded" => "Verbunden · Gerätedaten geladen",
        "Connection failed" => "Verbindung fehlgeschlagen",
        "Connection failed · {error}" => "Verbindung fehlgeschlagen · {error}",
        "LAST" => "LETZTE",
        "Base URL not configured" => "Basis-URL nicht konfiguriert",
        "Configured devices" => "Konfigurierte Geräte",
        "Panel text size" => "Panel-Schriftgröße",
        "Panel item spacing" => "Abstand der Panel-Elemente",
        "rain" => "Regen",
        "Move device up" => "Gerät nach oben verschieben",
        "Move device down" => "Gerät nach unten verschieben",
        "FRITZ!Box device unavailable" => "FRITZ!Box-Gerät nicht verfügbar",
        "FRITZ!Box devices unavailable" => "FRITZ!Box-Geräte nicht verfügbar",
        "High temperature" => "Hohe Temperatur",
        "Low temperature" => "Niedrige Temperatur",
        "maximum" => "Maximum",
        "minimum" => "Minimum",
        "Close (Esc)" => "Schließen (Esc)",
        "Open graph" => "Diagramm öffnen",
        "AVERAGE TEMPERATURE" => "DURCHSCHNITTSTEMPERATUR",
        "Forecast request in progress" => "Vorhersage wird abgerufen",
        "Forecast unavailable" => "Vorhersage nicht verfügbar",
        "Fritz Power Monitor notification test" => "Fritz Power Monitor-Benachrichtigungstest",
        "Get 7-day forecast" => "7-Tage-Vorhersage abrufen",
        "Japanese" => "Japanisch",
        "Korean" => "Koreanisch",
        "Notifications are working correctly." => "Benachrichtigungen funktionieren ordnungsgemäß.",
        "Open-Meteo · forecast" => "Open-Meteo · Vorhersage",
        "Open-Meteo · geocoding" => "Open-Meteo · Geokodierung",
        "Send test notification" => "Testbenachrichtigung senden",
        "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean." => {
            "Das System folgt der Sprache Ihres Desktops. Verfügbare Sprachen: Englisch, Deutsch, Spanisch, Niederländisch, Französisch, Italienisch, Schwedisch, Portugiesisch, Polnisch, Dänisch, Norwegisch, Japanisch und Koreanisch."
        }
        "TOTAL POWER INPUT" => "GESAMTE LEISTUNGSEINSPEISUNG",
        "TOTAL POWER OUTPUT" => "GESAMTE LEISTUNGSABGABE",
        "TOTAL ENERGY" => "GESAMTENERGIE",
        "Weather location not configured" => "Wetterstandort nicht konfiguriert",
        "Wikimedia Commons · image search and download" => {
            "Wikimedia Commons · Bildersuche und Download"
        }
        "Interface" => "Oberfläche",
        "Language and presentation" => "Sprache und Darstellung",
        "Display & updates" => "Anzeige und Aktualisierung",
        "Panel, graph history and refresh behavior" => {
            "Panel, Diagrammverlauf und Aktualisierungen"
        }
        "Data sources" => "Datenquellen",
        "FRITZ!Box devices and local weather information" => {
            "FRITZ!Box-Geräte und lokale Wetterinformationen"
        }
        "Layout" => "Anordnung",
        "Choose what appears and in which order" => {
            "Festlegen, was und in welcher Reihenfolge angezeigt wird"
        }
        "Support" => "Hilfe",
        "Connection health, diagnostics and version information" => {
            "Verbindungsstatus, Diagnose und Versionsinformationen"
        }
        "Welcome" => "Willkommen",
        "Connect your FRITZ!Box" => "FRITZ!Box verbinden",
        "Choose your devices" => "Geräte auswählen",
        "Optional weather" => "Optionales Wetter",
        "You're ready to go" => "Alles ist bereit",
        "Let's get your power monitor connected." => "Verbinden wir deinen Strommonitor.",
        "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings." => {
            "Diese kurze Einrichtung verbindet sich mit deiner FRITZ!Box, sucht kompatible Smart-Home-Geräte und richtet auf Wunsch lokale Wetterdaten ein. Du kannst alles später in den Einstellungen ändern."
        }
        "FRITZ!Box address" => "FRITZ!Box-Adresse",
        "http://fritz.box or https://fritz.box" => "http://fritz.box oder https://fritz.box",
        "Use the FRITZ!Box user credentials that have permission to access smart-home devices." => {
            "Verwende die Zugangsdaten eines FRITZ!Box-Benutzers mit Berechtigung für Smart-Home-Geräte."
        }
        "Discover compatible FRITZ! smart-home devices, then select the devices to monitor." => {
            "Suche nach kompatiblen FRITZ!-Smart-Home-Geräten und wähle anschließend die zu überwachenden Geräte aus."
        }
        "Discover devices" => "Geräte suchen",
        "Searching your FRITZ!Box…" => "FRITZ!Box wird durchsucht…",
        "No compatible devices were found. You can continue and retry later in Settings → Devices." => {
            "Keine kompatiblen Geräte gefunden. Du kannst fortfahren und die Suche später unter Einstellungen → Geräte erneut starten."
        }
        "Enable local weather (optional)" => "Lokales Wetter aktivieren (optional)",
        "Show weather" => "Wetter anzeigen",
        "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later." => {
            "Die Wetterdaten sind unabhängig von der FRITZ!Box-Überwachung. Du kannst diesen Schritt überspringen und später alles einrichten."
        }
        "Setup is complete. Your choices are saved automatically." => {
            "Die Einrichtung ist abgeschlossen. Deine Einstellungen werden automatisch gespeichert."
        }
        "devices selected" => "Geräte ausgewählt",
        "Back" => "Zurück",
        "Finish" => "Fertigstellen",
        "Finish setup" => "Einrichtung abschließen",
        "Continue" => "Weiter",
        "FRITZ!Box connection" => "FRITZ!Box-Verbindung",
        "Outdoor temperature source" => "Quelle der Außentemperatur",
        "OpenWeatherMap" => "OpenWeatherMap",
        "Use a selected FRITZ!Box device's sensor reading instead of the outdoor temperature reported by OpenWeatherMap." => "Verwende den Sensorwert eines ausgewählten FRITZ!Box-Geräts statt der von OpenWeatherMap gemeldeten Außentemperatur.",
        _ => key,
    }
}

#[cfg(test)]
mod tests {
    use super::{language_setting, tr};

    #[test]
    fn explicit_language_overrides_desktop_locale() {
        assert_eq!(tr("de", "Settings"), "Einstellungen");
        assert_eq!(tr("en", "Settings"), "Settings");
    }

    #[test]
    fn supported_languages_and_locale_variants_are_resolved() {
        for language in [
            "en", "de", "es", "nl", "fr", "it", "sv", "pt", "pl", "da", "no", "ja", "ko",
        ] {
            assert_eq!(super::language(language), language);
        }
        assert_eq!(super::language("fr_FR.UTF-8"), "fr");
        assert_eq!(super::language("nb_NO.UTF-8"), "no");
    }

    #[test]
    fn japanese_and_korean_locales_are_detected_and_translated() {
        for locale in ["ja", "ko"] {
            assert_eq!(super::language(locale), locale);
            for key in [
                "Settings",
                "Weather",
                "Connection",
                "Japanese",
                "Korean",
                "Panel item order",
                "FRITZ!Box URL",
                "Send test notification",
            ] {
                assert_ne!(tr(locale, key), key, "{locale}: {key}");
            }
        }
        assert_eq!(super::language("ja_JP.UTF-8"), "ja");
        assert_eq!(super::language("ko-KR"), "ko");
    }

    #[test]
    fn core_labels_are_translated_for_every_added_locale() {
        for language in ["es", "nl", "fr", "it", "sv", "pt", "pl", "da", "no"] {
            assert_ne!(tr(language, "Settings"), "Settings", "{language}");
            assert_ne!(tr(language, "Weather"), "Weather", "{language}");
            assert_ne!(tr(language, "Connection"), "Connection", "{language}");
        }
    }

    #[test]
    fn expanded_interface_and_diagnostics_are_translated_for_every_added_locale() {
        for language in ["es", "nl", "fr", "it", "sv", "pt", "pl", "da", "no"] {
            for key in [
                "Panel item order",
                "Show internet bandwidth",
                "FRITZ!Box URL",
                "Connection security",
                "Send test notification",
                "Get 7-day forecast",
                "Certificate validation disabled",
                "WEATHER UNAVAILABLE",
                "FRITZ!Box device unavailable",
                "e.g. Amsterdam or 52.37,4.90",
                "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.",
            ] {
                assert_ne!(tr(language, key), key, "{language}: {key}");
            }
        }
    }

    #[test]
    fn language_picker_names_are_localized_for_every_supported_locale() {
        let languages = [
            "English",
            "German",
            "Spanish",
            "Dutch",
            "French",
            "Italian",
            "Swedish",
            "Portuguese",
            "Polish",
            "Danish",
            "Norwegian",
            "Japanese",
            "Korean",
        ];
        for locale in [
            "de", "es", "nl", "fr", "it", "sv", "pt", "pl", "da", "no", "ja", "ko",
        ] {
            for name in languages {
                assert_ne!(tr(locale, name), name, "{locale}: {name}");
            }
        }
    }

    #[test]
    fn every_language_picker_choice_is_persisted_as_selected() {
        for language in [
            "system", "en", "de", "es", "nl", "fr", "it", "sv", "pt", "pl", "da", "no", "ja", "ko",
        ] {
            assert_eq!(language_setting(language), language);
        }
        assert_eq!(language_setting("unsupported"), "system");
    }

    #[test]
    fn unknown_translations_fall_back_to_source_text() {
        assert_eq!(tr("de", "A newly added label"), "A newly added label");
    }
    #[test]
    fn total_energy_label_is_translated_for_all_supported_languages() {
        for language in [
            "de", "es", "nl", "fr", "it", "sv", "pt", "pl", "da", "no", "ja", "ko",
        ] {
            assert_ne!(tr(language, "TOTAL ENERGY"), "TOTAL ENERGY", "{language}");
        }
    }

    #[test]
    fn every_app_interface_string_has_a_translation_for_each_supported_language() {
        let keys = [
            "FRITZ!Box connection",
            "Interface",
            "Language and presentation",
            "Display & updates",
            "Panel, graph history and refresh behavior",
            "Data sources",
            "FRITZ!Box devices and local weather information",
            "Layout",
            "Choose what appears and in which order",
            "Support",
            "Connection health, diagnostics and version information",
            "Welcome",
            "Connect your FRITZ!Box",
            "Choose your devices",
            "Optional weather",
            "You're ready to go",
            "Let's get your power monitor connected.",
            "This short setup will connect to your FRITZ!Box, discover compatible smart-home devices and optionally configure local weather. You can change everything later in Settings.",
            "FRITZ!Box address",
            "http://fritz.box or https://fritz.box",
            "Use the FRITZ!Box user credentials that have permission to access smart-home devices.",
            "Discover compatible FRITZ! smart-home devices, then select the devices to monitor.",
            "Discover devices",
            "Searching your FRITZ!Box…",
            "No compatible devices were found. You can continue and retry later in Settings → Devices.",
            "Enable local weather (optional)",
            "Show weather",
            "Weather is independent of FRITZ!Box monitoring. You can skip this and configure it later.",
            "Setup is complete. Your choices are saved automatically.",
            "devices selected",
            "Back",
            "Finish",
            "Finish setup",
            "Continue",
            "API key",
            "AVERAGE TEMPERATURE",
            "Adjusts the power and temperature text in the panel",
            "Average temperature",
            "Base URL not configured",
            "CONNECTION ERROR",
            "CURRENT POWER",
            "Clouds",
            "Combine consumption and solar power",
            "Configure your power monitor",
            "Configured devices",
            "Connected · device data loaded",
            "Connection",
            "Connection failed",
            "Connection security",
            "Controls the gap between panel items",
            "DOWNLOAD HISTORY",
            "Danish",
            "Device list and meter readings",
            "Display a separate refresh button beside the applet in the panel.",
            "Dutch",
            "Each photo is shown for 5 seconds to 5 minutes before the crossfade transition.",
            "English",
            "Exclude solar devices",
            "External resources",
            "FORECAST UNAVAILABLE",
            "FRITZ!Box device unavailable",
            "FRITZ!Box devices unavailable",
            "Feels like",
            "Forecast loaded",
            "Forecast request in progress",
            "Forecast unavailable",
            "French",
            "Fritz Power Monitor notification test",
            "GRAPH HISTORY",
            "GRAPH STYLE",
            "German",
            "Get 7-day forecast",
            "Graph",
            "HTTP failed",
            "HTTP successful",
            "HTTPS / TLS",
            "High temperature",
            "Humidity",
            "INTERNET BANDWIDTH",
            "Ignore temperatures from devices whose name contains “solar” when calculating the average.",
            "Image loaded",
            "Independent of FRITZ!Box refresh; default 10 minutes. Requires an API key and location.",
            "Interface language",
            "Italian",
            "Japanese",
            "Korean",
            "LAST",
            "Language",
            "Latest request failed",
            "Latest request succeeded",
            "Load devices",
            "Loading forecast…",
            "Location (city or latitude,longitude)",
            "Low temperature",
            "Lower limit",
            "Move device down",
            "Move device up",
            "Move down",
            "Move up",
            "Multiline panel values",
            "No usable image loaded",
            "Norwegian",
            "Not checked yet",
            "Not configured",
            "Notifications are working correctly.",
            "Open-Meteo · forecast",
            "Open-Meteo · geocoding",
            "OpenWeatherMap API key",
            "Outdoor temperature source",
            "OpenWeatherMap",
            "Use a selected FRITZ!Box device's sensor reading instead of the outdoor temperature reported by OpenWeatherMap.",
            "OpenWeatherMap · current weather",
            "Panel item spacing",
            "Panel text size",
            "Password",
            "Per-device switch state",
            "Photo",
            "Plain HTTP · TLS is not enabled",
            "Polish",
            "Portuguese",
            "Refresh",
            "Refresh now",
            "Reload now",
            "Request in progress",
            "Send test notification",
            "Session authentication",
            "Settings",
            "Show internet bandwidth",
            "Show panel refresh button",
            "Slideshow image duration",
            "Spanish",
            "Stack related readings vertically instead of placing them on one line.",
            "Sunrise",
            "Sunset",
            "Swedish",
            "System certificate roots",
            "System default",
            "System follows your desktop language. Available languages: English, German, Spanish, Dutch, French, Italian, Swedish, Portuguese, Polish, Danish, Norwegian, Japanese and Korean.",
            "TOTAL POWER INPUT",
            "TOTAL POWER OUTPUT",
            "UPLOAD HISTORY",
            "Upper limit",
            "Use current weather icon",
            "Username",
            "Visibility",
            "WEATHER UNAVAILABLE",
            "Weather location not configured",
            "Weather refresh",
            "Weather request failed",
            "Wikimedia Commons · image search and download",
            "e.g. Amsterdam or 52.37,4.90",
            "maximum",
            "minimum",
            "rain",
        ];
        for locale in [
            "de", "es", "nl", "fr", "it", "sv", "pt", "pl", "da", "no", "ja", "ko",
        ] {
            for key in keys {
                assert_ne!(tr(locale, key), key, "missing {locale} translation: {key}");
            }
        }
    }
}
