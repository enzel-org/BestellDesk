from flask import Flask, request, jsonify, render_template, session, redirect, url_for
from pymongo import MongoClient
from dotenv import load_dotenv
from bson.objectid import ObjectId
from datetime import datetime
import os

# .env laden
load_dotenv()

# Flask initialisieren
app = Flask(__name__)

# MongoDB verbinden
client = MongoClient(os.getenv("MONGODB_URI"))
db = client["bestellapp"]
bestellungen = db["bestellungen"]
lieferanten = db["lieferanten"]
einstellungen = db["einstellungen"]
app.secret_key = os.getenv("SECRET_KEY", "dev_default_key")

# Startseite
@app.route("/")
def bestellseite():
    lieferant = lieferanten.find_one({"aktiv": True})
    einstellung = einstellungen.find_one({"typ": "zeitfenster"})

    bestellbar = True
    hinweis = ""

    if einstellung:
        von = einstellung.get("von")
        bis = einstellung.get("bis")
        name = einstellung.get("name", "")
        now = datetime.now().time()

        try:
            von_time = datetime.strptime(von, "%H:%M").time()
            bis_time = datetime.strptime(bis, "%H:%M").time()
            bestellbar = von_time <= now <= bis_time
            hinweis = f"Bestellungen sind im Zeitraum {von} Uhr bis {bis} Uhr bei {name} möglich."
        except:
            bestellbar = True  # Falls Eingabe ungültig ist

    return render_template("bestellung.html", lieferant=lieferant, bestellbar=bestellbar, hinweis=hinweis)


# Bestellung absenden (API)
@app.route("/api/bestellung", methods=["POST"])
def bestellung_absenden():
    data = request.get_json()
    if not data:
        return jsonify({"status": "error", "reason": "no data"}), 400

    name = data.get("name")
    gerichte = data.get("gerichte", [])
    lieferant = lieferanten.find_one({"aktiv": True})

    if not name or not gerichte:
        return jsonify({"status": "error", "reason": "missing name or gerichte"}), 400

    bestellung = {
        "name": name,
        "gerichte": gerichte,
        "lieferant_id": lieferant["_id"] if lieferant else None
    }

    result = bestellungen.insert_one(bestellung)
    return jsonify({"status": "ok", "id": str(result.inserted_id)})

# Admin Login
@app.route("/admin", methods=["GET", "POST"])
def admin_login():
    fehler = None
    if request.method == "POST":
        benutzer = request.form.get("username")
        passwort = request.form.get("password")

        if benutzer == "admin" and passwort == "geheim":
            session["logged_in"] = True
            return redirect(url_for("admin_login"))
        else:
            fehler = "Falscher Benutzername oder Passwort"

    return render_template("admin.html", fehler=fehler)

@app.route("/admin/logout")
def admin_logout():
    session.clear()
    return redirect(url_for("admin_login"))

# Admin Bestellungen
@app.route("/admin/bestellungen")
def admin_bestellungen():
    if not session.get("logged_in"):
        return redirect(url_for("admin_login"))

    bestellungen_liste = list(bestellungen.find())
    gesamtpreis = 0

    for b in bestellungen_liste:
        summe = sum(g.get("preis", 0) for g in b.get("gerichte", []))
        b["summe"] = round(summe, 2)

        zahlung = b.get("zahlung", {})
        betrag = zahlung.get("betrag", 0)
        if not zahlung.get("rueckgeld_gegeben"):
            b["rueckgeld"] = round(betrag - summe, 2) if betrag > summe else 0
        else:
            b["rueckgeld"] = None

        gesamtpreis += summe

    return render_template("admin_bestellungen.html",
                           bestellungen=bestellungen_liste,
                           gesamtpreis=round(gesamtpreis, 2))

# Bestellung löschen
@app.route("/admin/bestellung/loeschen/<bestell_id>", methods=["POST"])
def bestellung_loeschen(bestell_id):
    if not session.get("logged_in"):
        return redirect(url_for("admin_login"))

    bestellungen.delete_one({"_id": ObjectId(bestell_id)})
    return redirect(url_for("admin_bestellungen"))

# Alle Bestellungen löschen
@app.route("/admin/bestellungen/alle-loeschen", methods=["POST"])
def bestellungen_loeschen_alle():
    if not session.get("logged_in"):
        return redirect(url_for("admin_login"))

    bestellungen.delete_many({})
    return redirect(url_for("admin_bestellungen"))

# Bestellung bearbeiten
@app.route("/admin/bestellung/bearbeiten/<bestell_id>", methods=["GET", "POST"])
def bestellung_bearbeiten(bestell_id):
    if not session.get("logged_in"):
        return redirect(url_for("admin_login"))

    bestellung = bestellungen.find_one({"_id": ObjectId(bestell_id)})
    if not bestellung:
        return "Bestellung nicht gefunden", 404

    if request.method == "POST":
        name = request.form.get("name")
        gerichte = []

        i = 0
        while True:
            prefix = f"gericht_{i}_"
            if f"{prefix}name" not in request.form:
                break
            gerichte.append({
                "nr": request.form.get(f"{prefix}nr"),
                "name": request.form.get(f"{prefix}name"),
                "preis": float(request.form.get(f"{prefix}preis")),
                "schaerfegrad": request.form.get(f"{prefix}schaerfegrad"),
                "notiz": request.form.get(f"{prefix}notiz")
            })
            i += 1

        bestellungen.update_one(
            {"_id": ObjectId(bestell_id)},
            {"$set": {"name": name, "gerichte": gerichte}}
        )
        return redirect(url_for("admin_bestellungen"))

    return render_template("admin_bearbeiten.html", bestellung=bestellung)

# Zahlung speichern
@app.route("/admin/bestellung/zahlung/<bestell_id>", methods=["POST"])
def bestellung_zahlung_speichern(bestell_id):
    if not session.get("logged_in"):
        return redirect(url_for("admin_login"))

    betrag_raw = request.form.get("betrag", "0").replace(",", ".").strip()
    try:
        betrag = float(betrag_raw) if betrag_raw else 0.0
    except ValueError:
        betrag = 0.0

    zahlung = {
        "erhalten": request.form.get("erhalten") == "on",
        "betrag": betrag,
        "rueckgeld_gegeben": request.form.get("rueckgeld_gegeben") == "on"
    }

    bestellungen.update_one(
        {"_id": ObjectId(bestell_id)},
        {"$set": {"zahlung": zahlung}}
    )

    return redirect(url_for("admin_bestellungen"))

# Lieferanten verwalten
@app.route("/admin/lieferanten", methods=["GET", "POST"])
def admin_lieferanten():
    if not session.get("logged_in"):
        return redirect(url_for("admin_login"))

    if request.method == "POST":
        name = request.form.get("name")
        versand = float(request.form.get("versand", 0))
        menu_typ = request.form.get("menu_typ")
        menu_info = request.form.get("menu_info")
        whatsapp = request.form.get("whatsapp")

        lieferanten.insert_one({
            "name": name,
            "versand_pro_person": versand,
            "menu_typ": menu_typ,
            "menu_info": menu_info,
            "whatsapp_nummer": whatsapp,
            "aktiv": False
        })
        return redirect(url_for("admin_lieferanten"))

    lieferanten_liste = list(lieferanten.find())
    einstellungen_dokument = einstellungen.find_one({"typ": "whatsapp"})
    aktuelle_nummer = einstellungen_dokument["nummer"] if einstellungen_dokument else None

    return render_template("admin_lieferanten.html", lieferanten=lieferanten_liste, aktuelle_nummer=aktuelle_nummer)

# Lieferant aktivieren
@app.route("/admin/lieferant/aktivieren/<lieferant_id>", methods=["POST"])
def lieferant_aktivieren(lieferant_id):
    if not session.get("logged_in"):
        return redirect(url_for("admin_login"))

    lieferanten.update_many({}, {"$set": {"aktiv": False}})
    lieferanten.update_one({"_id": ObjectId(lieferant_id)}, {"$set": {"aktiv": True}})

    aktiver = lieferanten.find_one({"_id": ObjectId(lieferant_id)})
    einstellungen.update_one(
        {"typ": "whatsapp"},
        {"$set": {"nummer": aktiver.get("whatsapp_nummer", "")}},
        upsert=True
    )

    return redirect(url_for("admin_lieferanten"))

# Lieferant löschen
@app.route("/admin/lieferant/loeschen/<lieferant_id>", methods=["POST"])
def lieferant_loeschen(lieferant_id):
    if not session.get("logged_in"):
        return redirect(url_for("admin_login"))

    lieferanten.delete_one({"_id": ObjectId(lieferant_id)})
    return redirect(url_for("admin_lieferanten"))

# Lieferant bearbeiten
@app.route("/admin/lieferant/bearbeiten/<lieferant_id>", methods=["GET", "POST"])
def lieferant_bearbeiten(lieferant_id):
    if not session.get("logged_in"):
        return redirect(url_for("admin_login"))

    lieferant = lieferanten.find_one({"_id": ObjectId(lieferant_id)})
    if not lieferant:
        return "Lieferant nicht gefunden", 404

    if request.method == "POST":
        name = request.form.get("name")
        versand = float(request.form.get("versand", 0))
        menu_typ = request.form.get("menu_typ")
        menu_info = request.form.get("menu_info")
        whatsapp = request.form.get("whatsapp")

        lieferanten.update_one(
            {"_id": ObjectId(lieferant_id)},
            {"$set": {
                "name": name,
                "versand_pro_person": versand,
                "menu_typ": menu_typ,
                "menu_info": menu_info,
                "whatsapp_nummer": whatsapp
            }}
        )
        return redirect(url_for("admin_lieferanten"))

    lieferant["whatsapp"] = lieferant.get("whatsapp_nummer", "")
    return render_template("admin_lieferant_bearbeiten.html", lieferant=lieferant)

# Admin Zeitfenster
@app.route("/admin/zeitfenster", methods=["GET", "POST"])
def admin_zeitfenster():
    if not session.get("logged_in"):
        return redirect(url_for("admin_login"))

    einstellung = einstellungen.find_one({"typ": "zeitfenster"})
    if request.method == "POST":
        von = request.form.get("von")
        bis = request.form.get("bis")
        name = request.form.get("name")

        einstellungen.update_one(
            {"typ": "zeitfenster"},
            {"$set": {
                "von": von,
                "bis": bis,
                "name": name
            }},
            upsert=True
        )
        return redirect(url_for("admin_zeitfenster"))

    return render_template("admin_zeitfenster.html", einstellung=einstellung)


# App starten
if __name__ == "__main__":
    app.run(debug=True)
