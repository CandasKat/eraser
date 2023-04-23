use std::env;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};
use chrono::{DateTime, Utc};
use walkdir::WalkDir;
use tempfile::tempdir;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
struct SilinenDosya {
    olusturulma_zamani: SystemTime,
    dosya_yolu: String,
}

type PaylasilanDosyaHaritasi = Arc<RwLock<HashMap<String, SilinenDosya>>>;


fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Kullanım: eraser <komut> <dosya_yolu>");
        return;
    }

    let paylasilan_dosya_haritasi: PaylasilanDosyaHaritasi = Arc::new(RwLock::new(HashMap::new()));
    let durma_sinyali = Arc::new(AtomicBool::new(false));

    let gozetmen_paylasilan_dosya_haritasi = Arc::clone(&paylasilan_dosya_haritasi);
    let gozetmen_durma_sinyali = Arc::clone(&durma_sinyali);
    let gozetmen = thread::spawn(move || dosya_gozetmeni(gozetmen_paylasilan_dosya_haritasi, gozetmen_durma_sinyali));

    let komut = &args[1];
    let dosya_yolu = Path::new(&args[2]);

    match komut.as_str() {
        "sil" => {
            match silme_yoneticisi(&dosya_yolu, &paylasilan_dosya_haritasi) {
                Ok(_) => println!("{} dosyası başarıyla silindi ve 24 saat sonra tamamen silinecek.", dosya_yolu.display()),
                Err(e) => eprintln!("Hata: {}", e),
            }
        }
        "geri_al" => {
            match geri_al(&dosya_yolu, &paylasilan_dosya_haritasi) {
                Ok(_) => println!("{} dosyası başarıyla geri alındı.", dosya_yolu
                    .display()),
                Err(e) => eprintln!("Hata: {}", e),
            }
        }
        _ => eprintln!("Geçersiz komut: {}", komut),
    }
// Programın sonunda durma sinyali gönderin
    durma_sinyali.store(true, Ordering::Relaxed);

// Gözetmen iş parçacığının bitmesini bekleyin
    gozetmen.join().unwrap();
}

fn silme_yoneticisi(dosya_yolu: &Path, paylasilan_dosya_haritasi: &PaylasilanDosyaHaritasi) -> Result<(), Box<dyn std::error::Error>> {
    let simdi = SystemTime::now();
    let gecici_dizin = tempdir()?;
    let gecici_dosya_yolu = gecici_dizin.path().join(dosya_yolu.file_name().unwrap());

    // Dosyayı geçici dizine taşı
    fs::rename(dosya_yolu, &gecici_dosya_yolu)?;

    // Dosya haritasına dosyayı ekle
    let mut dosya_haritasi = paylasilan_dosya_haritasi.write().unwrap();
    dosya_haritasi.insert(
        dosya_yolu.to_string_lossy().into_owned(),
        SilinenDosya {
            olusturulma_zamani: simdi,
            dosya_yolu: gecici_dosya_yolu.to_string_lossy().into_owned(),
        },
    );

    Ok(())
}


fn geri_al(dosya_yolu: &Path, paylasilan_dosya_haritasi: &PaylasilanDosyaHaritasi) -> Result<(), Box<dyn std::error::Error>> {
    let dosya_haritasi = paylasilan_dosya_haritasi.read().unwrap();
    let dosya = dosya_haritasi.get(&dosya_yolu.to_string_lossy().into_owned());

    match dosya {
        Some(silinen_dosya) => {
            fs::rename(&silinen_dosya.dosya_yolu, dosya_yolu)?;
            Ok(())
        }
        None => Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} dosyası bulunamadı.", dosya_yolu.display()),
        ))),
    }
}

fn dosya_gozetmeni(paylasilan_dosya_haritasi: PaylasilanDosyaHaritasi, durma_sinyali: Arc<AtomicBool>) {
    while !durma_sinyali.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_secs(60));

        let simdi = SystemTime::now();
        let dosya_haritasi = paylasilan_dosya_haritasi.read().unwrap();

        let gecersiz_dosya_yollari: Vec<String> = dosya_haritasi
            .iter()
            .filter_map(|(dosya_yolu, silinen_dosya)| {
                let dosya_yasi = simdi.duration_since(silinen_dosya.olusturulma_zamani).unwrap();
                let dosya_gecersiz = dosya_yasi >= Duration::from_secs(24 * 60 * 60);

                if dosya_gecersiz {
                    Some(dosya_yolu.clone())
                } else {
                    None
                }
            })
            .collect();

        drop(dosya_haritasi); // Read lock'ı bırakın

        for dosya_yolu in gecersiz_dosya_yollari {
            let mut dosya_haritasi = paylasilan_dosya_haritasi.write().unwrap();
            if let Some(silinen_dosya) = dosya_haritasi.remove(&dosya_yolu) {
                fs::remove_file(&silinen_dosya.dosya_yolu).unwrap();
            }
        }
    }
}
