rm(list=ls())
library(tidyverse)
library(readxl)
library(lubridate)
library(stringr)

remove_acentos <- function(x) iconv(x, to = "ASCII//TRANSLIT")

info_munic <- read.csv2('data/informacoes_municipais_seade.csv') %>%
  mutate(munic = tolower(munic),
         munic = remove_acentos(munic),
         munic = str_replace_all(munic, '-', ' '))

arquivo_xlsx <- 'data/Municipios informacoes dia.xlsx'

df <- excel_sheets(arquivo_xlsx) %>%
  map(function(x){
    tabela <- read_excel(arquivo_xlsx, x)

    if(length(tabela) < 3) {
      n <- str_which(tabela[[1]], "Cidade")

      tabela_casos <- tabela %>%
        slice(1:(n-2))

      names(tabela_casos) <- c('munic', 'casos')

      tabela_obitos <- tabela %>%
        slice((n+1):n())

      names(tabela_obitos) <- c('munic', 'obitos')

      tabela <- full_join(tabela_casos, tabela_obitos, by = 'munic')
    }

    tabela <- tabela %>%
      mutate(dia_mes = x,
             dia = as.numeric(substr(dia_mes, 1,2)),
             mes = substr(dia_mes, 4, 6),
             mes = replace(mes, mes == 'mar', 3),
             mes = replace(mes, mes == 'abr', 4),
             mes = as.numeric(mes)) %>%
      select(-dia_mes)

    names(tabela) <- c('munic', 'casos', 'obitos', 'dia', 'mes')

    tabela <- tabela %>%
      mutate(munic = tolower(munic),
             munic = remove_acentos(munic),
             munic = str_replace_all(munic, '-', ' '),
             munic = str_replace_all(munic, '\\?', ''),
             casos = as.numeric(casos),
             obitos = as.numeric(obitos)) %>%
      filter(!is.na(munic)) %>%
      filter(munic != 'total')

  })  %>%
  reduce(bind_rows) %>%
  left_join(info_munic, by = 'munic')

df %>%
  write_csv2('data/dados_covid_sp.csv')

tail(df)
