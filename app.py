from flask import Flask, request, jsonify, render_template, session, redirect, url_for
from pymongo import MongoClient
from dotenv import load_dotenv
from bson.objectid import ObjectId
import os

# .env laden
load_dotenv()

# Flask initialisieren
app = Flask(__name__)

# MongoDB verbinden
client = MongoClient(os.getenv("MONGODB_URI"))
db = client["bestellapp"]
bestellungen = db["bestellungen"]
app.secret_key = os.getenv("SECRET_KEY", "dev_default_key")


# Standard Website (Frontend)
@app.route("/")
def bestellseite():
    return render_template("bestellung.html")

# Benutzer Seite
@app.route("/api/bestellung", methods=["POST"])
def bestellung_absenden():
    data = request.get_json()
    if not data:
        return jsonify({"status": "error", "reason": "no data"}), 400

    name = data.get("name")
    gerichte = data.get("gerichte", [])

    if not name or not gerichte:
        return jsonify({"status": "error", "reason": "missing name or gerichte"}), 400

    bestellung = {
        "name": name,
        "gerichte": gerichte
    }

    result = bestellungen.insert_one(bestellung)
    return jsonify({"status": "ok", "id": str(result.inserted_id)})

# Admin Seite
@app.route("/admin", methods=["GET", "POST"])
def admin_login():
    fehler = None
    if request.method == "POST":
        benutzer = request.form.get("username")
        passwort = request.form.get("password")

        # Temporärer Login, später aus DB/Einstellungsdatei
        if benutzer == "admin" and passwort == "geheim":
            session["logged_in"] = True
            return redirect(url_for("admin_login"))
        else:
            fehler = "Falscher Benutzername oder Passwort"

    return render_template("admin.html", fehler=fehler)

# Admin Logout
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
    return render_template("admin_bestellungen.html", bestellungen=bestellungen_liste)

# Admin Bestellung Löschen
@app.route("/admin/bestellung/loeschen/<bestell_id>", methods=["POST"])
def bestellung_loeschen(bestell_id):
    if not session.get("logged_in"):
        return redirect(url_for("admin_login"))

    bestellungen.delete_one({"_id": ObjectId(bestell_id)})
    return redirect(url_for("admin_bestellungen"))

# Admin alle Bestellungen Löschen
@app.route("/admin/bestellungen/alle-loeschen", methods=["POST"])
def bestellungen_loeschen_alle():
    if not session.get("logged_in"):
        return redirect(url_for("admin_login"))

    bestellungen.delete_many({})
    return redirect(url_for("admin_bestellungen"))

# App starten
if __name__ == "__main__":
    app.run(debug=True)
