from flask import Flask, request, jsonify
from pymongo import MongoClient
from dotenv import load_dotenv
import os

# .env laden
load_dotenv()

# Flask initialisieren
app = Flask(__name__)

# MongoDB verbinden
client = MongoClient(os.getenv("MONGODB_URI"))
db = client["bestellapp"]
bestellungen = db["bestellungen"]

# Beispielroute: Bestellung speichern
@app.route("/api/bestellung", methods=["POST"])
def bestellung_absenden():
    daten = request.json
    if not daten or "name" not in daten or "gerichte" not in daten:
        return jsonify({"error": "Ungültige Daten"}), 400

    gerichte = []
    for gericht in daten["gerichte"]:
        if not all(k in gericht for k in ("nr", "name", "preis")):
            return jsonify({"error": "Ungültige Gerichtsdaten"}), 400
        eintrag = {
            "nr": gericht["nr"],
            "name": gericht["name"],
            "preis": gericht["preis"]
        }
        # Optionaler Schärfegrad
        if "schaerfegrad" in gericht:
            eintrag["schaerfegrad"] = gericht["schaerfegrad"]
        gerichte.append(eintrag)

    bestellung = {
        "name": daten["name"],
        "gerichte": gerichte
    }

    result = bestellungen.insert_one(bestellung)
    return jsonify({"status": "ok", "id": str(result.inserted_id)}), 201


# App starten
if __name__ == "__main__":
    app.run(debug=True)
